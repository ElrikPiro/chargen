use chargen_app::{ContainerBuilder, ServiceConfig};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.first().is_some_and(|arg| arg == "bench") {
        return chargen_app::run_bench_cli(&args[1..]).map_err(|error| error.to_string());
    }
    if args
        .first()
        .is_some_and(|arg| arg == "--help" || arg == "-h")
    {
        println!(
            "{}\n\nComandos:\n  serve [opciones]\n  bench --profile B0|B1|B2 [--database RUTA]",
            chargen_app::command_usage()
        );
        return Ok(());
    }
    let config = ServiceConfig::from_args(&args).map_err(|error| format!("{error:?}"))?;
    let container = ContainerBuilder::new(config)
        .build()
        .map_err(|error| format!("{error:?}"))?;
    container.serve().await.map_err(|error| error.to_string())?;
    Ok(())
}
