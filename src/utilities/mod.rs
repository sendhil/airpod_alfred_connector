use super::bluetooth::DeviceInfo;
use json::{self, object};

pub fn print_alfred_output(devices: Vec<DeviceInfo>) {
    let mut data = json::JsonValue::new_array();

    for device in devices {
        let mut title = format!("{} (Connected)", device.name);
        if !device.connected {
            title = device.name;
        }

        data.push(object! {
            type: "default",
            title: title,
            subtitle: format!("MAC:{}", device.address),
            arg: device.address,
        })
        .expect("Error generating output for Alfred");
    }

    let items = object! {
        items: data
    };

    println!("{}", items.dump());
}

pub fn device_list_from_cli_arg(device_list: &str) -> Option<Vec<String>> {
    if device_list.len() == 0 {
        return None;
    }

    let results = device_list
        .split(",")
        .map(|x| x.to_string())
        .collect::<Vec<String>>();

    match results.len() {
        0 => None,
        _ => Some(results),
    }
}
