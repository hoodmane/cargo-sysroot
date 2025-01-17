//! # Cargo-Sysroot
//!
//! Compiles the Rust sysroot crates, core, compiler_builtins, and alloc.
//!
//! Cargo.toml package.metadata.cargo-sysroot.target should be set
//! to the path of a Target Specification
//!
//! The sysroot is located in `.target/sysroot`
use anyhow::*;
use cargo_toml2::{from_path, to_path, Build, CargoConfig, CargoToml};
use std::{fs, path::Path};
use structopt::StructOpt;

mod args;
#[allow(dead_code)]
mod util;
use crate::{args::*, util::get_rust_src};
use cargo_sysroot_2::*;

/// Create a `.cargo/config` to use our target and sysroot.
fn generate_cargo_config(target: &Path, sysroot: &Path) -> Result<()> {
    let cargo = Path::new(".cargo");
    let cargo_config = cargo.join("config.toml");
    fs::create_dir_all(cargo)?;

    if cargo_config.exists() {
        // TODO: Be smarter, update existing. Warn?
        return Ok(());
    }

    let target = target
        // .canonicalize()
        // .with_context(|| {
        //     format!(
        //         "Couldn't get absolute path to custom target: {}",
        //         target.display()
        //     )
        // })?
        .to_str()
        .context("Failed to convert target.json path to utf-8")?
        .to_string();
    let sysroot_dir = sysroot
        .canonicalize()
        .with_context(|| {
            format!(
                "Couldn't get canonical path to sysroot: {}",
                sysroot.display()
            )
        })?
        .to_str()
        .with_context(|| {
            format!(
                "Failed to convert sysroot path to utf-8: {}",
                sysroot.display()
            )
        })?
        .to_string();

    let config = CargoConfig {
        build: Some(Build {
            target: Some(target),
            rustflags: Some(vec!["--sysroot".to_owned(), sysroot_dir]),
            ..Default::default()
        }),
        ..Default::default()
    };
    if !cargo_config.exists() {
        to_path(&cargo_config, &config).context("Failed writing sysroot Cargo.toml")?;
    }

    Ok(())
}

fn main() -> Result<()> {
    let Args::Sysroot(mut args) = Args::from_args();
    let manifest_path = args.manifest_path.clone().unwrap_or("./Cargo.toml".into());
    let toml: Result<CargoToml> =
        from_path(&manifest_path).with_context(|| manifest_path.display().to_string());

    if args.target.is_none() {
        args.target = Some(
            toml?.package
                .metadata
                .context("Missing package metadata")?
                .get("cargo-sysroot")
                .context("Missing cargo-sysroot metadata")?
                .get("target")
                .context("Missing cargo-sysroot target")?
                .as_str()
                .context("Cargo-sysroot target field was not a string")?
                .into(),
        );
    }

    if args.rust_src_dir.is_none() {
        args.rust_src_dir = Some(get_rust_src()?)
    }

    clean_artifacts(&args.sysroot_dir)?;
    fs::create_dir_all(&args.sysroot_dir).context("Couldn't create sysroot directory")?;

    let args = args;

    println!("Building sysroot crates");
    if !args.no_config {
        generate_cargo_config(args.target.as_ref().unwrap(), &args.sysroot_dir)
            .context("Couldn't create .cargo/config.toml")?;
    }

    let mut sys = SysrootBuilder::new(cargo_sysroot_2::Sysroot::Alloc);
    if let Some(path) = args.manifest_path {
        sys.manifest(path);
    }
    sys.output(args.sysroot_dir)
        .target(args.target.expect("BUG: Missing target triple?"))
        .features(&[Features::CompilerBuiltinsMem]);
    if let Some(rust_src) = args.rust_src_dir {
        sys.rust_src(rust_src);
    }
    sys.build()?;

    Ok(())
}
