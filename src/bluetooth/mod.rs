use std::{error::Error, fmt, process::Command, str};

use log::trace;

use mockall::*;

use lazy_static::lazy_static;

use regex::Regex;
#[derive(Debug, PartialEq)]
pub struct DeviceInfo {
    pub name: String,
    pub address: String,
    pub connected: bool,
}

impl DeviceInfo {
    fn from_raw_str(data: &str) -> DeviceInfo {
        lazy_static! {
            static ref RE: Regex =
                Regex::new(r#"^address: ([a-zA-Z0-9_-]{17}),.*name: "([^"]*)""#).unwrap();
        }

        assert!(
            RE.is_match(&data),
            "Regex to match Bluetooth info shouldn't fail. Failed to match : {}",
            data
        );

        let mut name: String = Default::default();
        let mut address: String = Default::default();
        let connected: bool = !data.contains("not connected");
        for cap in RE.captures_iter(data) {
            name = cap.get(2).map_or("", |m| m.as_str()).to_string();
            address = cap.get(1).map_or("", |m| m.as_str()).to_string();
            break;
        }

        return DeviceInfo {
            name,
            address,
            connected,
        };
    }
}

#[derive(Debug, PartialEq)]
pub enum DeviceFilters {
    AllDevices,
    SpecificAddresses { addresses: Vec<String> },
    Regex { value: String },
}

pub struct DeviceListOptions {
    filters: DeviceFilters,
    previous_address: Option<String>,
}

impl DeviceListOptions {
    pub fn new(filters: DeviceFilters, previous_address: Option<String>) -> Self {
        DeviceListOptions {
            filters,
            previous_address,
        }
    }

    fn new_default_all_devices() -> Self {
        return DeviceListOptions {
            filters: DeviceFilters::AllDevices,
            previous_address: None,
        };
    }
}

pub struct BluetoothClient {
    blueutil_client: Box<dyn Client>,
}

impl BluetoothClient {
    pub fn new() -> Self {
        BluetoothClient {
            blueutil_client: Box::new(BlueutilClient::new()),
        }
    }

    pub fn connect_to_device(&self, address: &str) -> Result<(), Box<dyn Error>> {
        self.blueutil_client.connect_to_device(address)
    }

    pub fn disconnect_from_device(&self, address: &str) -> Result<(), Box<dyn Error>> {
        self.blueutil_client.disconnect_from_device(address)
    }

    // bool indicates that the device was connected to.
    pub fn toggle_connected_status(&self, address: &str) -> Result<bool, Box<dyn Error>> {
        let device = self.get_device_info(address)?;

        if device.connected {
            self.disconnect_from_device(address)?;
            Ok(false)
        } else {
            self.connect_to_device(address)?;
            Ok(true)
        }
    }

    pub fn get_device_list(&self, options: DeviceListOptions) -> Vec<DeviceInfo> {
        let mut devices = self.blueutil_client.get_device_list();
        devices = self.get_filtered_devices(devices, options.filters);

        devices.sort_by(|a, b| b.connected.cmp(&a.connected));

        // Move last used device to the top
        if let Some(previous_address) = options.previous_address {
            devices.sort_by_key(|a| a.address.to_lowercase() != previous_address.to_lowercase());
        }

        devices
    }

    pub fn print_devices(&self) {
        let parsed_devices = self.get_device_list(DeviceListOptions::new_default_all_devices());

        for parsed_device in parsed_devices {
            println!("{:#?}", parsed_device);
        }
    }

    pub fn is_device_connected(&self, address: &str) -> Result<bool, BluetoothClientError> {
        let device = self.get_device_info(address)?;

        Ok(device.connected)
    }

    fn get_filtered_devices(
        &self,
        devices: Vec<DeviceInfo>,
        filters: DeviceFilters,
    ) -> Vec<DeviceInfo> {
        match filters {
            DeviceFilters::AllDevices => devices,
            DeviceFilters::SpecificAddresses { addresses } => devices
                .into_iter()
                .filter(|x| addresses.contains(&x.address.to_lowercase()))
                .collect(),
            DeviceFilters::Regex { value } => devices
                .into_iter()
                .filter(|x| x.name.to_lowercase().contains(&value))
                .collect(),
        }
    }

    fn get_device_info(&self, address: &str) -> Result<DeviceInfo, BluetoothClientError> {
        let device_list_options = DeviceListOptions::new(
            DeviceFilters::SpecificAddresses {
                addresses: vec![address.to_string()],
            },
            None,
        );
        let mut filtered_device = self
            .get_device_list(device_list_options)
            .into_iter()
            .filter(|x| x.address.to_lowercase() == address.to_lowercase())
            .collect::<Vec<DeviceInfo>>();

        if filtered_device.len() == 0 {
            Err(BluetoothClientError::new(&format!(
                "Could not find device id : '{}'",
                address
            )))
        } else {
            Ok(filtered_device.remove(0))
        }
    }
}

#[derive(Debug)]
pub struct BluetoothClientError {
    details: String,
}

impl BluetoothClientError {
    fn new(msg: &str) -> BluetoothClientError {
        BluetoothClientError {
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for BluetoothClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for BluetoothClientError {
    fn description(&self) -> &str {
        &self.details
    }
}

pub trait Client {
    fn connect_to_device(&self, address: &str) -> Result<(), Box<dyn Error>>;
    fn disconnect_from_device(&self, address: &str) -> Result<(), Box<dyn Error>>;
    fn get_device_list(&self) -> Vec<DeviceInfo>;
}

struct BlueutilClient {
    command_runner: Box<dyn CommandRunner>,
}

#[automock]
impl Client for BlueutilClient {
    fn connect_to_device(&self, address: &str) -> Result<(), Box<dyn Error>> {
        let output = self.run_command(vec!["--connect", &address]);

        trace!("{:?}", &output.stdout);
        trace!("{:?}", &output.stderr);

        Ok(())
    }

    fn disconnect_from_device(&self, address: &str) -> Result<(), Box<dyn Error>> {
        let output = self.run_command(vec!["--disconnect", &address, "--info", &address]);

        trace!("{:?}", &output.stdout);
        trace!("{:?}", &output.stderr);

        Ok(())
    }

    fn get_device_list(&self) -> Vec<DeviceInfo> {
        let output = self.run_command(vec!["--paired"]);

        let results = str::from_utf8(&output.stdout).unwrap();

        results
            .split("\n")
            .filter(|x| x.len() > 0)
            .map(|x| DeviceInfo::from_raw_str(x))
            .collect()
    }
}

impl BlueutilClient {
    fn new() -> Self {
        BlueutilClient {
            command_runner: Box::new(DefaultCommandRunner {}),
        }
    }

    fn run_command(&self, args: Vec<&str>) -> std::process::Output {
        self.command_runner.run_command(
            &self.get_blueutil_path(),
            args.into_iter().map(|x| x.to_string()).collect(),
        )
    }

    fn get_blueutil_path(&self) -> String {
        match std::env::var("BLUEUTIL_PATH") {
            Ok(val) => format!("{}/blueutil", val),
            Err(_) => String::from("blueutil"),
        }
    }
}

//
#[automock]
trait CommandRunner {
    fn run_command(&self, command: &str, args: Vec<String>) -> std::process::Output;
}

struct DefaultCommandRunner {}
impl CommandRunner for DefaultCommandRunner {
    fn run_command(&self, command: &str, args: Vec<String>) -> std::process::Output {
        Command::new(command)
            .args(args)
            .output()
            .expect("There was an error running blueutil")
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt;

    use super::*;

    #[test]
    fn device_info_parses_raw_str() {
        let valid_str_not_connected = r#"address: 5c-2e-fg-da-a3-43, not connected, not favourite, paired, name: "AirPods Pro", recent access date: 2022-08-01 12:00:10 +0000"#;
        let valid_str_connected = r#"address: 80-3b-5c-c2-b1-7f, connected (master, 0 dBm), not favourite, paired, name: "AirPods Max", recent access date: 2022-08-01 12:10:10 +0000"#;

        let valid_device_not_connected = DeviceInfo::from_raw_str(valid_str_not_connected);
        assert_eq!(valid_device_not_connected.name, "AirPods Pro");
        assert_eq!(valid_device_not_connected.address, "5c-2e-fg-da-a3-43");
        assert_eq!(valid_device_not_connected.connected, false);

        let valid_device_connected = DeviceInfo::from_raw_str(valid_str_connected);
        assert_eq!(valid_device_connected.name, "AirPods Max");
        assert_eq!(valid_device_connected.address, "80-3b-5c-c2-b1-7f");
        assert_eq!(valid_device_connected.connected, true);
    }

    #[test]
    #[should_panic]
    fn device_info_panics_for_invalid_str() {
        let invalid_str = "address: 5c-2e-fg-da-a3-43";
        DeviceInfo::from_raw_str(invalid_str);
    }

    #[test]
    fn dev_device_list_options_constructor() {
        let result = DeviceListOptions::new(DeviceFilters::AllDevices, Some(String::from("1234")));

        assert_eq!(result.filters, DeviceFilters::AllDevices);
        assert_eq!(result.previous_address, Some(String::from("1234")))
    }

    #[test]
    fn dev_device_list_default_all_devices() {
        let result = DeviceListOptions::new_default_all_devices();

        assert_eq!(result.filters, DeviceFilters::AllDevices);
        assert_eq!(result.previous_address, None);
    }

    #[test]
    fn bluetooth_client_print_devices_retrieves_device_list() {
        let mut mock = MockBlueutilClient::default();
        mock.expect_get_device_list().times(1).returning(|| vec![]);

        let client = BluetoothClient {
            blueutil_client: Box::new(mock),
        };

        client.print_devices();
    }

    #[test]
    fn bluetooth_client_connect_to_device() {
        let mut mock = MockBlueutilClient::default();
        mock.expect_connect_to_device()
            .times(1)
            .with(predicate::eq("address"))
            .returning(|_| Ok(()));

        let client = BluetoothClient {
            blueutil_client: Box::new(mock),
        };

        client.connect_to_device("address").unwrap();
    }

    #[test]
    fn bluetooth_client_disconnect_from_device() {
        let mut mock = MockBlueutilClient::default();
        mock.expect_disconnect_from_device()
            .times(1)
            .with(predicate::eq("address"))
            .returning(|_| Ok(()));

        let client = BluetoothClient {
            blueutil_client: Box::new(mock),
        };

        client.disconnect_from_device("address").unwrap();
    }

    #[test]
    fn bluetooth_client_toggle_connected_status_disconnects_a_connected_device() {
        let mut mock = MockBlueutilClient::default();
        mock_blueutil_client_device_list(&mut mock);

        mock.expect_connect_to_device()
            .times(1)
            .returning(|_| Ok(()));
        mock.expect_disconnect_from_device()
            .times(0)
            .returning(|_| Ok(()));

        let client = BluetoothClient {
            blueutil_client: Box::new(mock),
        };

        client
            .toggle_connected_status("disconnected-address")
            .unwrap();
    }

    #[test]
    fn bluetooth_client_toggle_connected_status_connects_a_disconnected_device() {
        let mut mock = MockBlueutilClient::default();
        mock_blueutil_client_device_list(&mut mock);

        mock.expect_connect_to_device()
            .times(0)
            .returning(|_| Ok(()));
        mock.expect_disconnect_from_device()
            .times(1)
            .returning(|_| Ok(()));

        let client = BluetoothClient {
            blueutil_client: Box::new(mock),
        };

        client.toggle_connected_status("connected-address").unwrap();
    }

    #[test]
    fn bluetooth_client_get_device_list_calls_client() {
        let mut mock = MockBlueutilClient::default();
        mock.expect_get_device_list().times(1).returning(|| vec![]);

        let client = BluetoothClient {
            blueutil_client: Box::new(mock),
        };

        client.get_device_list(DeviceListOptions::new_default_all_devices());
    }

    #[test]
    fn bluetooth_client_get_device_list_filters_all() {
        let mut mock = MockBlueutilClient::default();
        mock_blueutil_client_device_list(&mut mock);

        let client = BluetoothClient {
            blueutil_client: Box::new(mock),
        };

        let devices = client.get_device_list(DeviceListOptions {
            filters: DeviceFilters::AllDevices,
            previous_address: None,
        });
        let all_devices = blueutil_default_client_list();

        assert_eq!(devices.len(), all_devices.len());
        assert!(devices.contains(&all_devices[0]));
        assert!(devices.contains(&all_devices[1]));
        assert!(devices.contains(&all_devices[2]));
    }

    #[test]
    fn bluetooth_client_get_device_list_filters_regex() {
        let mut mock = MockBlueutilClient::default();
        mock_blueutil_client_device_list(&mut mock);

        let client = BluetoothClient {
            blueutil_client: Box::new(mock),
        };

        let devices = client.get_device_list(DeviceListOptions {
            filters: DeviceFilters::Regex {
                value: String::from("device1"),
            },
            previous_address: None,
        });
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "device1");
        assert_eq!(devices[0].address, "disconnected-address");
        assert_eq!(devices[0].connected, false);
    }

    #[test]
    fn bluetooth_client_get_device_list_filters_specific_address() {
        let mut mock = MockBlueutilClient::default();
        mock_blueutil_client_device_list(&mut mock);

        let client = BluetoothClient {
            blueutil_client: Box::new(mock),
        };

        let devices = client.get_device_list(DeviceListOptions {
            filters: DeviceFilters::SpecificAddresses {
                addresses: vec![String::from("connected-address-2")],
            },
            previous_address: None,
        });
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "device3");
        assert_eq!(devices[0].address, "connected-address-2");
        assert_eq!(devices[0].connected, true);
    }

    #[test]
    fn bluetooth_client_get_device_list_filters_specific_addresses() {
        let mut mock = MockBlueutilClient::default();
        mock_blueutil_client_device_list(&mut mock);

        let client = BluetoothClient {
            blueutil_client: Box::new(mock),
        };

        let devices = client.get_device_list(DeviceListOptions {
            filters: DeviceFilters::SpecificAddresses {
                addresses: vec![
                    String::from("connected-address"),
                    String::from("connected-address-2"),
                ],
            },
            previous_address: None,
        });
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "device2");
        assert_eq!(devices[1].name, "device3");
    }

    #[test]
    fn bluetooth_client_get_device_list_moves_previous_address_to_top() {
        let mut mock = MockBlueutilClient::default();
        mock_blueutil_client_device_list(&mut mock);

        let client = BluetoothClient {
            blueutil_client: Box::new(mock),
        };

        vec![
            String::from("connected-address"),
            String::from("connected-address-2"),
            String::from("disconnected-address"),
        ]
        .into_iter()
        .for_each(|address| {
            let devices = client.get_device_list(DeviceListOptions {
                filters: DeviceFilters::AllDevices,
                previous_address: Some(address.clone()),
            });

            assert_eq!(devices.len(), 3);
            assert_eq!(devices[0].address, address);
        });
    }

    #[test]
    fn bluetooth_client_is_device_connected() {
        let mut mock = MockBlueutilClient::default();
        mock_blueutil_client_device_list(&mut mock);

        let client = BluetoothClient {
            blueutil_client: Box::new(mock),
        };

        assert_eq!(
            client.is_device_connected("connected-address").unwrap(),
            true,
        );

        assert_eq!(
            client.is_device_connected("disconnected-address").unwrap(),
            false,
        );
    }

    #[test]
    fn blueutil_client_connect_to_device() {
        let mut mock = MockCommandRunner::default();

        mock.expect_run_command()
            .withf(|command, args| command == "blueutil" && args.eq(&vec!["--connect", "address"]))
            .times(1)
            .returning(|_, _| std::process::Output {
                status: ExitStatusExt::from_raw(0),
                stdout: Default::default(),
                stderr: Default::default(),
            });

        let client = BlueutilClient {
            command_runner: Box::new(mock),
        };

        client.connect_to_device("address").unwrap();
    }

    #[test]
    fn blueutil_client_disconnect_from_device() {
        let mut mock = MockCommandRunner::default();

        mock.expect_run_command()
            .withf(|command, args| {
                command == "blueutil"
                    && args.eq(&vec!["--disconnect", "address", "--info", "address"])
            })
            .times(1)
            .returning(|_, _| std::process::Output {
                status: ExitStatusExt::from_raw(0),
                stdout: Default::default(),
                stderr: Default::default(),
            });

        let client = BlueutilClient {
            command_runner: Box::new(mock),
        };

        client.disconnect_from_device("address").unwrap();
    }

    #[test]
    fn blueutil_client_get_device_list() {
        let mut mock = MockCommandRunner::default();

        mock.expect_run_command()
            .withf(|command, args| command == "blueutil" && args.eq(&vec!["--paired"]))
            .times(1)
            .returning(|_, _| std::process::Output {
                status: ExitStatusExt::from_raw(0),
                stdout: Default::default(),
                stderr: Default::default(),
            });

        let client = BlueutilClient {
            command_runner: Box::new(mock),
        };

        client.get_device_list();
    }

    fn mock_blueutil_client_device_list(mock: &mut MockBlueutilClient) {
        mock.expect_get_device_list()
            .returning(|| blueutil_default_client_list());
    }

    fn blueutil_default_client_list() -> Vec<DeviceInfo> {
        vec![
            DeviceInfo {
                name: String::from("device1"),
                address: String::from("disconnected-address"),
                connected: false,
            },
            DeviceInfo {
                name: String::from("device2"),
                address: String::from("connected-address"),
                connected: true,
            },
            DeviceInfo {
                name: String::from("device3"),
                address: String::from("connected-address-2"),
                connected: true,
            },
        ]
    }
}
