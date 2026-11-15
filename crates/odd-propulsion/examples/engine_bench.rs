//! Print an engine design's performance.
//!
//! ```sh
//! cargo run -p odd-propulsion --example engine_bench -- lh2 5.5 3.5e6 0.030 80 0.8 0
//! ```
//! Arguments: propellant (`lh2|rp1|lch4|mmh`), O/F, chamber pressure Pa,
//! throat radius m, expansion ratio, bell length fraction, ambient Pa.
use odd_propulsion::{nozzle, solve_engine, EngineSpec, Propellant};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 6 {
        eprintln!("usage: engine_bench <lh2|rp1|lch4|mmh> <O/F> <Pc Pa> <Rt m> <eps> <length fraction> [ambient Pa]");
        std::process::exit(2);
    }
    let propellant = match args[0].as_str() {
        "lh2" => Propellant::LoxLh2,
        "rp1" => Propellant::LoxRp1,
        "lch4" => Propellant::LoxLch4,
        "mmh" => Propellant::NtoMmh,
        other => {
            eprintln!("unknown propellant {other}");
            std::process::exit(2);
        }
    };
    let number = |index: usize| -> f64 { args[index].parse().expect("numeric argument") };
    let expansion: f64 = number(4);
    let spec = EngineSpec {
        propellant,
        of_ratio: number(1),
        p_c_pa: number(2),
        r_throat_m: number(3),
        r_exit_m: number(3) * expansion.sqrt(),
        length_fraction: number(5),
    };
    let ambient = args.get(6).map_or(0.0, |value| value.parse().unwrap());
    match solve_engine(&spec, ambient) {
        Ok(s) => {
            println!(
                "{} O/F {:.3} Pc {:.3} MPa eps {:.1}",
                propellant.label(),
                spec.of_ratio,
                spec.p_c_pa / 1e6,
                expansion
            );
            println!(
                "  thrust {:.3} N   Isp {:.4} s   mdot {:.6} kg/s",
                s.thrust_n, s.specific_impulse_s, s.mass_flow_kg_s
            );
            println!(
                "  dry mass {:.4} kg   pump {:.3} MW   c* {:.2} m/s",
                s.dry_mass_kg,
                s.pump_power_w / 1e6,
                s.c_star_m_s
            );
            println!(
                "  Tc {:.1} K   M {:.3} g/mol   gamma {:.4}   rho_bulk {:.3} kg/m3",
                s.chamber_temperature_k,
                s.molar_mass_kg_mol * 1000.0,
                s.gamma,
                s.bulk_density_kg_m3
            );
            println!(
                "  exit M {:.4}  Pe {:.1} Pa  Te {:.1} K  lambda {:.5}",
                s.exit_mach, s.exit_pressure_pa, s.exit_temperature_k, s.divergence_factor
            );
            println!(
                "  throat q {:.4} MW/m2  wall {:.1} K  limit {}",
                s.throat_heat_flux_w_m2 / 1e6,
                s.throat_wall_temperature_k,
                s.thermal_limit_exceeded
            );
            println!(
                "  chamber r {:.4} m  L {:.4} m  nozzle L {:.4} m",
                s.chamber_radius_m, s.chamber_length_m, s.nozzle_length_m
            );
            match s.separation {
                Some(sep) => println!(
                    "  SEPARATED at x/L {:.4}, eps {:.3}, side load {:.3} kN",
                    sep.x_norm, sep.area_ratio_sep, sep.side_load_kn
                ),
                None => println!("  attached at {:.1} Pa ambient", ambient),
            }
            let _ = nozzle::ambient_pressure(0.0);
        }
        Err(error) => {
            eprintln!("rejected: {error}");
            std::process::exit(1);
        }
    }
}
