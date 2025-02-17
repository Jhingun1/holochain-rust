use crate::context::Context;
use std::sync::Arc;
use crate::state_dump::{StateDump, address_to_content_and_type};
use holochain_core_types::chain_header::ChainHeader;
use holochain_persistence_api::cas::content::{AddressableContent, Address};

fn header_to_string(h: &ChainHeader) -> String {
    format!(
r#"===========Header===========
Type: {:?}
Timestamp: {}
Sources: {:?}
Header address: {}
Prev. address: {:?}
----------Content----------"#,
    h.entry_type(),
    h.timestamp(),
    h.provenances()
        .iter()
        .map(|p| p.source().to_string())
        .collect::<Vec<String>>()
        .join(", "),
    h.address(),
    h.link().map(|l| l.to_string()))
}

fn address_to_content_string(address: &Address, context: Arc<Context>) -> String {
    let maybe_content = address_to_content_and_type(address, context.clone());
    maybe_content
        .map(|(content_type, content)| {
            format!("* [{}] {}: {}", content_type, address.to_string(), content)
        })
        .unwrap_or_else(|err| {
            format!(
                "* [UNKNOWN] {}: Error trying to get type/content: {}",
                address.to_string(),
                err
            )
        })
}

pub fn state_dump(context: Arc<Context>) {
    let dump = StateDump::from(context.clone());

    let pending_validation_strings = dump.pending_validations
        .iter()
        .map(|pending_validation| {
            let maybe_content =
                address_to_content_and_type(&pending_validation.address, context.clone());
            maybe_content
                .map(|(content_type, content)| {
                    format!(
                        "<{}> [{}] {}: {}",
                        pending_validation.workflow.to_string(),
                        content_type,
                        pending_validation.address.to_string(),
                        content
                    )
                })
                .unwrap_or_else(|err| {
                    format!(
                        "<{}> [UNKNOWN] {}: Error trying to get type/content: {}",
                        pending_validation.workflow.to_string(),
                        pending_validation.address.to_string(),
                        err
                    )
                })
        })
        .collect::<Vec<String>>();

    let holding_strings = dump.held_entries
        .iter()
        .map(|address| address_to_content_string(address, context.clone()))
        .collect::<Vec<String>>();

    let source_chain_strings = dump.source_chain
        .iter()
        .map(|h| format!(
            "{}\n=> {}",
            header_to_string(h),
            address_to_content_string(h.entry_address(), context.clone())
        ))
        .collect::<Vec<String>>();

    let debug_dump = format!(
        r#"
=============STATE DUMP===============
Agent's Source Chain:
========

{source_chain}

Nucleus:
========
Running zome calls: {calls:?}
-------------------
Pending validations:
{validations}
--------------------

Network:
--------
Running query flows: {flows:?}
------------------------
Running VALIDATION PACKAGE requests: {validation_packages:?}
------------------------------------
Running DIRECT MESSAGES: {direct_messages:?}

Dht:
====
Holding:
{holding_list}
--------
    "#,
        source_chain = source_chain_strings.join("\n\n"),
        calls = dump.running_calls,
        validations = pending_validation_strings.join("\n"),
        flows = dump.query_flows,
        validation_packages = dump.validation_package_flows,
        direct_messages = dump.direct_message_flows,
        holding_list = holding_strings.join("\n")
    );

    log_info!(context, "debug/state_dump: {}", debug_dump);
}
