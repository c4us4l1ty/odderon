//! What the authority actually accepts, stated once and pinned.
//!
//! The prose in `docs/server-authority.md` and `docs/software-design.md` said
//! "new submissions require FDR schema 3". The engine does not accept schema 3
//! for a new flight: it requires `SCHEMA_VERSION` (4) and rejects anything
//! older as a verification service rather than a rating. That distinction is
//! load-bearing — a v3 record carries no coupled propellant state, so
//! certifying it as a new ranked flight would score a model the authority no
//! longer flies — so it belongs in a test, not only in a comment.
//!
//! The recorder's own guards run *before* the engine's, which is why the
//! version-only cases below are asserted at the recorder as well: a record
//! that mislabels itself cannot even be serialized, let alone submitted.

mod common;

use common::{commit_for, engine, prediction_a, signed_rescue, submit_request};

/// Shipped, re-signable fixtures from the frozen recorder builds.
const PRE_CRYOGENIC: [&str; 2] = [
    include_str!("../../odd-verify/tests/fixtures/v1-rescue.txt"),
    include_str!("../../odd-verify/tests/fixtures/v2-rescue.txt"),
];

#[test]
fn a_new_submission_must_carry_the_current_schema() {
    assert_eq!(
        odd_report::SCHEMA_VERSION,
        4,
        "the coupled v4 law is what new flights are certified under"
    );

    let (fdr, license) = signed_rescue(prediction_a());
    let mut authority = engine();
    let ticket = commit_for(&mut authority, &fdr, &license);
    let response = authority
        .submit(submit_request(&fdr, ticket), 1_000)
        .expect("a current-schema flight is accepted");
    assert!(response.accepted);

    // The same flight relabelled v3 is refused before any rating, even though
    // it is still a perfectly valid signed v3 record.
    let mut older = fdr.clone();
    older.schema_version = odd_report::FROZEN_V3_SCHEMA_VERSION;
    older.sign(&license).expect("a v3 record still signs");

    let mut second = engine();
    let ticket = commit_for(&mut second, &older, &license);
    let error = second
        .submit(submit_request(&older, ticket), 1_000)
        .expect_err("a v3 record cannot be submitted as a new flight");
    assert!(
        matches!(error, odd_worker::ServerError::MalformedFdr(_)),
        "expected a schema refusal, got {error:?}"
    );
}

#[test]
fn a_pre_cryogenic_record_can_never_earn_a_receipt() {
    // Schemas 1 and 2 stay readable and replayable offline, but they predate
    // the tank and delegation proof. They are authentic signed records, so the
    // refusal has to come from the authority, not from a parse error.
    for payload in PRE_CRYOGENIC {
        let body = payload
            .strip_prefix("#verify=")
            .expect("fixtures are #verify= fragments");
        let fdr = odd_report::Fdr::from_verify_payload(body).expect("the fixture parses");
        assert!(
            fdr.schema_version < odd_report::FROZEN_V3_SCHEMA_VERSION,
            "fixture {} must predate the cryogenic schema",
            fdr.schema_version
        );
        assert!(
            fdr.verify_signature().is_ok(),
            "the fixture carries a valid pilot signature"
        );

        // The commitment is signed by whoever submits; this test is about the
        // record's schema, so the submitting identity does not matter.
        let mut authority = engine();
        let license = odd_identity::License::from_entropy([21u8; 16]);
        let ticket = commit_for(&mut authority, &fdr, &license);
        let error = authority
            .submit(submit_request(&fdr, ticket), 1_000)
            .expect_err("no receipt for a pre-cryogenic record");
        assert!(
            matches!(error, odd_worker::ServerError::MalformedFdr(_)),
            "schema {} must be refused by the authority, got {error:?}",
            fdr.schema_version
        );
    }
}

#[test]
fn the_recorder_refuses_to_mislabel_a_record() {
    // A future schema is not something this build can serialize, so the guard
    // fires at the recorder rather than at the network edge.
    let (mut fdr, license) = signed_rescue(prediction_a());
    fdr.schema_version = odd_report::SCHEMA_VERSION + 1;
    assert!(
        fdr.sign(&license).is_err(),
        "an unknown future schema must not be signable"
    );

    // A schema-1 record may not carry an engine block it cannot have had.
    let (mut legacy, license) = signed_rescue(prediction_a());
    legacy.schema_version = 1;
    let error = legacy
        .sign(&license)
        .expect_err("a legacy record must reject the engine block");
    assert!(
        matches!(error, odd_report::FdrError::Malformed(_)),
        "expected the legacy guard, got {error:?}"
    );
}
