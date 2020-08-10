use crate::common::commit_wireguard_configuration;
use crate::common::generate_wireguard_keys;
use crate::common::read_all_used_ipv4;
use crate::common::read_server_key;
use crate::common::render_all_entries;
use crate::common::render_server_config_head;
use crate::config::*;
use crate::templates::WireguardWorkstationTemplate;
use crate::utils::find_last_ipv4;
use crate::utils::first_of_pair;
use crate::utils::next_workstation_ipv4;
use crate::utils::write_atomic;
use crate::ENTRIES_DIR;
use crate::SERVER_PUBLIC_KEY;
use crate::WORKSTATIONS_DIR;
use askama::Template;
use std::{fs::read_to_string, path::Path};


#[post("/<name>")]
pub fn new(name: String) -> String {
    let (private_key, public_key) = generate_wireguard_keys();

    // if IP entry with given name already exists - we wish to re-use it:
    let existing_entry =
        Path::new(&format!("{}{}{}", ENTRIES_DIR, WORKSTATIONS_DIR, name)).to_owned();
    let user_ipv4 = if existing_entry.exists() {
        let line = read_to_string(existing_entry).unwrap_or_default();
        first_of_pair(&line).unwrap_or_default()
    } else {
        let all_used_ipv4s = read_all_used_ipv4(WORKSTATIONS_DIR);
        let last_ipv4 = match find_last_ipv4(all_used_ipv4s) {
            Some(ipv4) => ipv4,
            None => format!("{}.1.1", config().main_net), /* if list of entries is empty, assign next address after router */
        };
        match next_workstation_ipv4(&last_ipv4) {
            Some(ipv4) => ipv4,
            None => panic!("Address pool exhausted!"),
        }
    };

    // store entry for user with new generated pubkey:
    write_atomic(
        &format!("{}{}{}", ENTRIES_DIR, WORKSTATIONS_DIR, name),
        &format!("{},{}", user_ipv4, public_key),
    );

    let user_nets = format!("{}{}", user_ipv4, config().main_net_mask);

    // server main template
    let server_template = render_server_config_head();

    // iterate over all entries, build public side of server-side wireguard server configuration
    let server_config_entries_rendered = render_all_entries();

    let server_config_rendered = format!(
        "{}\n\n\n{}\n",
        server_template, server_config_entries_rendered
    );

    // write altered server configuration:
    write_atomic(&config().wireguard_conf, &server_config_rendered);

    // commit changes to system
    commit_wireguard_configuration(&user_ipv4);

    let user_template = WireguardWorkstationTemplate {
        user_name: &name,
        user_private_key: &private_key,
        user_nets: &user_nets,
        server_public_key: &read_server_key(SERVER_PUBLIC_KEY),
        default_server_endpoint: &format!(
            "{}:{}",
            config().server_public_ip,
            config().server_port
        ),
    };

    format!("{}\n", user_template.render().unwrap_or_default())
}
