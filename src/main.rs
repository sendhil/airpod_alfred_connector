use std::env;

use airpod_alfred_connector::bluetooth::DeviceFilters;
use clap::Parser;
use clap::Subcommand;

use airpod_alfred_connector::bluetooth::{self, DeviceListOptions};
use airpod_alfred_connector::utilities;

#[derive(Debug, Parser)]
#[clap(name = "airpod-alfred-bluetooth")]
#[clap(about = "Utility to simplify connecting/disconnecting to Airpods from Alfred")]
struct Cli {
    #[clap(subcommand)]
    command: Commands,

    #[clap(flatten)]
    verbose: clap_verbosity_flag::Verbosity,
}

#[derive(Debug, Subcommand)]
enum Commands {
    // Lists Airpods
    List {
        #[clap(short)]
        all_devices: Option<bool>,
        #[clap(short)]
        device_list: Option<String>,
    },
    #[clap(arg_required_else_help = true)]
    // Connects to an Airpod
    Connect {
        device_id: String,
    },
    // Disconnects from an Airpod
    #[clap(arg_required_else_help = true)]
    Disconnect {
        device_id: String,
    },
    // Toggles Connection to Airpod
    Toggle {
        device_id: String,
    },
}

fn main() {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    // Workflow saves the previously selected mac address into this env variable
    let previous_address = match env::var("AIRPODS_MAC") {
        Ok(val) => Some(val),
        Err(_) => None,
    };

    let client = bluetooth::BluetoothClient::new();

    match cli.command {
        Commands::List {
            all_devices,
            device_list,
        } => {
            let mut filter = match all_devices {
                Some(all_devices) if all_devices => DeviceFilters::AllDevices,
                _ => DeviceFilters::Regex {
                    value: String::from("airpod"), // TODO - Figure out a better default
                },
            };

            if let Some(device_list) = device_list {
                if let Some(device_list) = utilities::device_list_from_cli_arg(&device_list) {
                    filter = DeviceFilters::SpecificAddresses {
                        addresses: device_list,
                    }
                }
            }

            let devices = client.get_device_list(DeviceListOptions::new(filter, previous_address));

            utilities::print_alfred_output(devices);
        }
        Commands::Connect { device_id } => match client.connect_to_device(&device_id) {
            Ok(_) => println!("Connected to device"),
            Err(err) => eprintln!("{}", err),
        },
        Commands::Disconnect { device_id } => match client.disconnect_from_device(&device_id) {
            Ok(_) => println!("Disconnected from device"),
            Err(err) => eprintln!("{}", err),
        },
        Commands::Toggle { device_id } => match client.toggle_connected_status(&device_id) {
            Ok(connected) => {
                if connected {
                    println!("connected");
                } else {
                    println!("disconnected");
                }
            }
            Err(err) => eprintln!("{}", err),
        },
    }
}
