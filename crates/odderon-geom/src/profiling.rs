//! Triangle and duration accounting for the geometry kit.
//!
//! These are tests, not examples: they exist so a change to the kit can be
//! judged by a number instead of by a feeling, and so "this part got cheaper"
//! can be checked part by part. Run them with
//!
//! ```text
//! cargo test -p odderon-geom -- --nocapture
//! cargo test --release -p odderon-geom build_cost -- --nocapture
//! ```
//!
//! Everything here is `#[cfg(test)]`, so none of it reaches the wasm.

use crate::modules;
use crate::rooms::{self, IntPal};
use crate::*;
use std::sync::Mutex;

/// The part-by-part accounting reads a shared counter, so the two tests that
/// use it must not run at the same time as each other (or as any other test
/// that composes a building).
static PROBE_LOCK: Mutex<()> = Mutex::new(());

/// Build something into a throwaway buffer and return (triangles, vertices).
fn tris(f: impl FnOnce(&mut MeshBuffer)) -> (usize, usize) {
    let mut b = MeshBuffer::new();
    f(&mut b);
    (b.indices.len() / 3, b.verts.len() / FLOATS_PER_VERTEX)
}

#[test]
fn static_breakdown() {
    let (hab, hv) = tris(|b| hab::build_hab_204_static(b));
    let (town, tv) = tris(|b| town::build_uchinoura_street_static(b));
    std::println!("[profile] hab   {hab:6} tris {hv:6} verts");
    std::println!("[profile] town  {town:6} tris {tv:6} verts");
    std::println!("[profile] total {} tris", hab + town);
    let mut k = MeshBuffer::new();
    modules::building(
        &mut k,
        &modules::BuildingSpec {
            width: 8.0,
            depth: 6.8,
            floors: 2,
            floor_h: 2.8,
            seed: 0x5EED_0505,
            pal: modules::Pal::MACHIYA,
            skin: modules::Skin::BoardOverPlaster,
            upper_skin: None,
            ground: modules::GroundFloor::House,
            roof: modules::RoofKind::Kawara,
            balcony: true,
            wear: 0.5,
            detail: 1,
        },
    );
    std::println!("[profile] one kit building {} tris", k.indices.len() / 3);
    let base = modules::BuildingSpec {
        width: 8.0,
        depth: 6.8,
        floors: 2,
        floor_h: 2.8,
        seed: 0x5EED_0505,
        pal: modules::Pal::MACHIYA,
        skin: modules::Skin::BoardOverPlaster,
        upper_skin: None,
        ground: modules::GroundFloor::House,
        roof: modules::RoofKind::Kawara,
        balcony: true,
        wear: 0.5,
        detail: 1,
    };
    let _guard = PROBE_LOCK.lock().unwrap();
    let _ = modules::probe::take();
    {
        let mut p = MeshBuffer::new();
        modules::building(&mut p, &modules::BuildingSpec::default());
        let mut prev = 0;
        for (l, t) in modules::probe::take() {
            std::println!("[part-of] {l:18} {:5}", t.saturating_sub(prev));
            prev = t;
        }
    }
    let mut costs: Vec<(&str, usize)> = Vec::new();
    {
        let mut a = MeshBuffer::new();
        modules::building(&mut a, &base);
        let mut b = MeshBuffer::new();
        modules::building(&mut b, &modules::BuildingSpec { detail: 0, ..base.clone() });
        let mut c = MeshBuffer::new();
        modules::building(&mut c, &modules::BuildingSpec { balcony: false, ..base.clone() });
        let mut d = MeshBuffer::new();
        modules::building(&mut d, &modules::BuildingSpec { roof: modules::RoofKind::Parapet, ..base.clone() });
        let mut e = MeshBuffer::new();
        modules::building(&mut e, &modules::BuildingSpec { ground: modules::GroundFloor::OpenBay, ..base.clone() });
        let mut f = MeshBuffer::new();
        modules::building(&mut f, &modules::BuildingSpec { floors: 1, ..base.clone() });
        let full = a.indices.len() / 3;
        costs.push(("full", full));
        costs.push(("detail0", b.indices.len() / 3));
        costs.push(("no balcony", (full as i64 - (c.indices.len() / 3) as i64) as usize));
        costs.push(("kawara roof", (full as i64 - (d.indices.len() / 3) as i64) as usize));
        costs.push(("ground openings", (full as i64 - (e.indices.len() / 3) as i64) as usize));
        costs.push(("upper storey", (full as i64 - (f.indices.len() / 3) as i64) as usize));
    }
    for (n, v) in costs {
        std::println!("[cost] {n:16} {v:6}");
    }
}

fn t(name: &str, f: impl FnOnce(&mut MeshBuffer)) {
    let mut b = MeshBuffer::new();
    f(&mut b);
    std::println!("[street] {name:22} {:5}", b.indices.len() / 3);
}

#[test]
fn street_breakdown() {
    t("grass strip 4.8/26", |b| props::summer_grass_strip(b, Pose::at(0.0, 0.0, 0.0), 4.8, 26, 101));
    t("grass strip 3.2/20", |b| props::summer_grass_strip(b, Pose::at(0.0, 0.0, 0.0), 3.2, 20, 203));
    t("flowering shrub", |b| props::flowering_roadside_shrub(b, Pose::at(0.0, 0.0, 0.0)));
    t("utility pole", |b| town::build_japanese_utility_pole(b, Pose::at(0.0, 0.0, 0.0), true));
    t("vending machine", |b| props::japanese_vending_machine(b, Pose::at(0.0, 0.0, 0.0)));
    t("bus stop", |b| props::bus_stop_shelter(b, Pose::at(0.0, 0.0, 0.0)));
    t("bicycle", |b| props::parked_bicycle(b, Pose::at(0.0, 0.0, 0.0)));
    t("planter", |b| props::terracotta_planter(b, Pose::at(0.0, 0.0, 0.0)));
    t("ac condenser", |b| props::outdoor_ac_condenser(b, Pose::at(0.0, 0.0, 0.0)));
}

fn t2(name: &str, f: impl FnOnce(&mut MeshBuffer)) {
    let mut b = MeshBuffer::new();
    f(&mut b);
    std::println!("[room] {name:16} {:5}", b.indices.len() / 3);
}

#[test]
fn room_breakdown() {
    let pal = IntPal::HAB;
    t2("tatami 2.8x2.8", |b| rooms::tatami_floor(b, 0.0, 2.8, 0.0, 2.8, &pal, 7));
    t2("plank 3.2x2.6", |b| rooms::plank_deck(b, 0.0, 3.2, 0.0, 2.6, 0.0, 0.024, true, &pal, 3));
    t2("shoji panel 0.9x1.8", |b| rooms::shoji_panel(b, 0.9, 1.8, &pal, 5));
    t2("shoji run 4", |b| rooms::shoji_run(b, 3.2, 1.8, 4, &pal, 5));
    t2("oshiire", |b| rooms::oshiire(b, 1.7, 1.9, &pal, 5));
    t2("shelf_run", |b| rooms::shelf_run(b, 1.8, 1.4, 0.30, 4, &pal, 5));
    t2("curtain pair", |b| rooms::curtain_pair(b, 2.4, 2.2, 0.55, 0.12, 0.0, &pal, [0.6, 0.55, 0.46]));
    t2("plant", |b| rooms::plant(b, 0.0, 0.0, 1.0, &pal, 5));
    t2("pendant", |b| rooms::pendant_lamp(b, 0.0, 2.4, 0.7, 0.0, 0.16, &pal));
    t2("table", |b| rooms::low_table(b, 0.0, 0.0, 1.2, 0.7, 0.34, &pal));
    t2("cushion", |b| rooms::cushion(b, 0.0, 0.0, 0.5, 0.5, [0.5, 0.2, 0.2]));
    t2("wall scroll", |b| rooms::wall_scroll(b, 0.45, 1.1, &pal, 5));
}

fn t3(name: &str, f: impl FnOnce(&mut MeshBuffer)) {
    let mut b = MeshBuffer::new();
    f(&mut b);
    std::println!("[part] {:22} {:5} tris", name, b.indices.len() / 3);
}

#[test]
fn part_breakdown() {
    let pal = modules::Pal::MACHIYA;
    let mut r = modules::Rng::new(1);
    t3("wall(8m, board/plast)", |b| {
        modules::wall_open(b, 8.0, 0.0, 2.74, modules::Skin::BoardOverPlaster, &pal, &mut modules::Rng::new(1), &[])
    });
    t3("wall(8m, corrugated)", |b| {
        modules::wall_open(b, 8.0, 0.0, 2.74, modules::Skin::Corrugated, &pal, &mut modules::Rng::new(1), &[])
    });
    t3("wall(8m, stone)", |b| {
        modules::wall_open(b, 8.0, 0.0, 2.74, modules::Skin::Stone, &pal, &mut modules::Rng::new(1), &[])
    });
    t3("wall+2 holes", |b| {
        modules::wall_open(b, 8.0, 0.0, 2.74, modules::Skin::BoardOverPlaster, &pal, &mut modules::Rng::new(1), &[modules::Hole::new(-2.0, -0.8, 0.7, 2.3), modules::Hole::new(1.0, 2.2, 0.1, 2.2)])
    });
    for st in [modules::WindowStyle::Sash, modules::WindowStyle::Lattice, modules::WindowStyle::Casement, modules::WindowStyle::Shopfront, modules::WindowStyle::Shoji] {
        t(&format!("window {st:?}"), |b| modules::window_unit(b, 0.0, 1.5, 0.9, 2.3, st, &pal, &mut modules::Rng::new(2)));
    }
    for ds in [modules::DoorStyle::ShopGlass, modules::DoorStyle::Plank, modules::DoorStyle::RollerShutter, modules::DoorStyle::Noren] {
        t(&format!("door {ds:?}"), |b| modules::door_unit(b, 0.0, 1.2, 0.1, 2.1, ds, &pal, &mut modules::Rng::new(3)));
    }
    t3("modules::eave(8m)", |b| modules::eave(b, 8.0, 2.74, 0.6, &pal, &mut modules::Rng::new(4), true));
    t3("modules::eave(8m,no rafters)", |b| modules::eave(b, 8.0, 2.74, 0.6, &pal, &mut modules::Rng::new(4), false));
    t3("modules::roof_kawara(8x6.8)", |b| modules::roof_kawara(b, 8.0, 5.6, 6.8, 1.2, &pal));
    t3("roof_corrugated", |b| modules::roof_corrugated(b, 8.0, 5.6, 6.8, &pal));
    t3("modules::balcony(6m)", |b| modules::balcony(b, 6.0, 2.74, 0.9, &pal, &mut modules::Rng::new(5)));
    t3("downpipe", |b| modules::downpipe(b, 3.0, 0.15, 5.5, &pal));
    t3("band", |b| modules::band(b, 8.0, 2.74, &pal));
    t3("ac_unit", |b| modules::ac_unit(b, 0.0, 2.0, &mut modules::Rng::new(6)));
    t3("lantern", |b| modules::lantern(b, 0.0, 2.4));
    t3("awning", |b| modules::awning(b, 6.0, 2.6, 0.7, pal.accent, pal.accent));
    t3("sign_plate", |b| modules::sign_plate(b, 0.0, 2.4, 1.0, 0.35, pal.trim, &pal));
    t3("meter_box", |b| modules::meter_box(b, 0.0, 1.2, &pal, &mut modules::Rng::new(7)));
    t3("vent", |b| modules::vent(b, 0.0, 2.2, 0.34, 0.24, &pal));
    let _ = r.next_u32();
}

#[test]
fn build_cost() {
    let mut b = MeshBuffer::new();
    // Warm the allocator and the CPU, then time 50 full static-world builds.
    for _ in 0..5 {
        b.clear();
        build_static_world(&mut b);
    }
    let t0 = std::time::Instant::now();
    let n = 400;
    for _ in 0..n {
        b.clear();
        build_static_world(&mut b);
    }
    let dt = t0.elapsed().as_secs_f64() / n as f64;
    std::println!("[build] static world {:.3} ms/build  ({} tris)", dt * 1e3, b.indices.len() / 3);
}