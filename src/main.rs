use clap::{Parser, Subcommand, ValueEnum};
use sensconv::{cm_per_360, convert, find, load, Game, Method, Setup};
use std::process::ExitCode;

const BUNDLED: &str = include_str!("../data/games.json");

#[derive(Parser)]
#[command(version, about = "Convert mouse sensitivity between games")]
struct Cli {
    /// Use this games file instead of the bundled one
    #[arg(long, global = true, value_name = "FILE")]
    games: Option<std::path::PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum Kind {
    Cm360,
    Fov,
}

#[derive(Subcommand)]
enum Cmd {
    /// List the known games and their yaw values
    List,
    /// Print cm/360 for one game
    Cm360 {
        #[arg(long)]
        game: String,
        #[arg(long)]
        sens: f64,
        #[arg(long, default_value_t = 800.0)]
        dpi: f64,
    },
    /// Convert a sensitivity from one game to another
    Convert {
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        sens: f64,
        /// Mouse DPI for the source game
        #[arg(long, default_value_t = 800.0)]
        dpi: f64,
        /// Mouse DPI for the destination game (defaults to --dpi)
        #[arg(long)]
        to_dpi: Option<f64>,
        #[arg(long, value_enum, default_value = "cm360")]
        method: Kind,
        /// FOV match point: 0 is screen centre, 1 is the screen edge (0.75 is a common choice)
        #[arg(long, default_value_t = 0.75)]
        coefficient: f64,
        /// Source horizontal FOV in degrees (defaults to the game's fixed FOV)
        #[arg(long)]
        from_fov: Option<f64>,
        /// Destination horizontal FOV in degrees (defaults to the game's fixed FOV)
        #[arg(long)]
        to_fov: Option<f64>,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn positive(name: &str, v: f64) -> Result<f64, String> {
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(format!("{name} must be a positive number"))
    }
}

fn game<'a>(games: &'a [Game], id: &str) -> Result<&'a Game, String> {
    find(games, id).ok_or_else(|| format!("unknown game '{id}', run `sensconv list`"))
}

fn hfov(given: Option<f64>, g: &Game, flag: &str) -> Result<f64, String> {
    let v = given
        .or(g.default_hfov)
        .ok_or_else(|| format!("{} has no fixed FOV, pass {flag} in degrees", g.name))?;
    if v > 0.0 && v < 180.0 {
        Ok(v)
    } else {
        Err(format!("{flag} must be between 0 and 180 degrees"))
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let json = match &cli.games {
        Some(p) => std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?,
        None => BUNDLED.to_string(),
    };
    let games = load(&json).map_err(|e| e.to_string())?;
    match cli.cmd {
        Cmd::List => {
            println!("{:<11} {:<34} {:>11}  fixed hFOV", "id", "name", "yaw");
            for g in &games {
                let fov = g.default_hfov.map_or("-".to_string(), |f| f.to_string());
                println!("{:<11} {:<34} {:>11}  {}", g.id, g.name, g.yaw, fov);
            }
        }
        Cmd::Cm360 { game: id, sens, dpi } => {
            let g = game(&games, &id)?;
            let cm = cm_per_360(positive("sens", sens)?, positive("dpi", dpi)?, g.yaw);
            println!(
                "{} sens {sens} at {dpi} DPI: {cm:.2} cm/360 ({:.2} in/360)",
                g.name,
                cm / 2.54
            );
        }
        Cmd::Convert {
            from,
            to,
            sens,
            dpi,
            to_dpi,
            method,
            coefficient,
            from_fov,
            to_fov,
        } => {
            let (src, dst) = (game(&games, &from)?, game(&games, &to)?);
            let to_dpi = to_dpi.unwrap_or(dpi);
            let method = match method {
                Kind::Cm360 => Method::Cm360,
                Kind::Fov => {
                    if !(0.0..=1.0).contains(&coefficient) {
                        return Err("--coefficient must be between 0 and 1".into());
                    }
                    Method::Fov {
                        src_hfov: hfov(from_fov, src, "--from-fov")?,
                        dst_hfov: hfov(to_fov, dst, "--to-fov")?,
                        coefficient,
                    }
                }
            };
            let c = convert(
                Setup {
                    yaw: src.yaw,
                    dpi: positive("dpi", dpi)?,
                },
                Setup {
                    yaw: dst.yaw,
                    dpi: positive("to-dpi", to_dpi)?,
                },
                positive("sens", sens)?,
                method,
            );
            println!(
                "{:<10} {:>10} sens at {} DPI = {:>7.2} cm/360",
                src.id, sens, dpi, c.src_cm360
            );
            println!(
                "{:<10} {:>10.4} sens at {} DPI = {:>7.2} cm/360",
                dst.id, c.dst_sens, to_dpi, c.dst_cm360
            );
        }
    }
    Ok(())
}
