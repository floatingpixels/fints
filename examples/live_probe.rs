use std::{
    env,
    error::Error,
    fmt::Write as _,
    io::{self, Write as _},
    process,
};

use chrono::{Local, NaiveDateTime};
use encoding_rs::mem::decode_latin1;
use fints::{
    AdvertisedCapabilitySnapshot, BalanceRequest, BankResponse, Client, ContinuationKind,
    Credentials, CreditCardBalanceRequest, CreditCardTransactionRequest, DepotPositionRequest,
    Initialization, InstituteId, OperationCapabilitySnapshot, ProductIdentity, ReusableState,
    SecuritiesTransactionRequest, Synchronization, Tan, TanMethod, TraceDirection, TraceEvent,
    TraceSink,
};
type ProbeResult<T> = Result<T, Box<dyn Error>>;

fn main() {
    if let Err(error) = run() {
        println!("probe failed: {error}");
        process::exit(1);
    }
}

fn run() -> ProbeResult<()> {
    if !enabled("FINTS_LIVE_PROBE") {
        return Err(input_error(
            "refusing to run without the explicit FINTS_LIVE_PROBE=1 gate",
        ));
    }

    let endpoint = required_input("FINTS_ENDPOINT", "FinTS HTTPS endpoint")?;
    let blz = required_input("FINTS_BLZ", "Institute code / BLZ")?;
    let user_id = required_input("FINTS_USER_ID", "User ID")?;
    let pin = required_input("FINTS_PIN", "PIN (stdin input may be echoed)")?;
    let customer_id = env::var("FINTS_CUSTOMER_ID")
        .ok()
        .filter(|value| !value.is_empty());
    let product_id = required_input("FINTS_PRODUCT_ID", "Registered product ID")?;
    let product_version = required_input("FINTS_PRODUCT_VERSION", "Registered product version")?;
    let country_code = env::var("FINTS_COUNTRY_CODE").unwrap_or_else(|_| "280".to_owned());

    let trace_sink = enabled("FINTS_LIVE_TRACE").then(raw_stdout_trace);
    let mut client = Client::new_with_trace(
        &endpoint,
        InstituteId::new(country_code, blz)?,
        ProductIdentity::new(product_id, product_version)?,
        Credentials::new(user_id, customer_id, pin)?,
        ReusableState::new(),
        trace_sink,
    )?;

    let session = probe_session(&mut client);
    let termination = client.terminate(now());
    print_bank_responses(&client, "terminate");
    session?;
    termination?;
    println!("probe status=complete");
    Ok(())
}

fn probe_session(client: &mut Client) -> ProbeResult<()> {
    initialize_until_connected(client)?;
    print_capabilities(&client.advertised_capabilities());

    if let Some(account) = account_flag("FINTS_PROBE_BALANCE_ACCOUNT")? {
        probe_balance(client, account)?;
    }
    if let Some(account) = account_flag("FINTS_PROBE_DEPOT_POSITIONS_ACCOUNT")? {
        probe_depot_positions(client, account)?;
    }
    if let Some(account) = account_flag("FINTS_PROBE_DEPOT_TRANSACTIONS_ACCOUNT")? {
        probe_depot_transactions(client, account)?;
    }
    if let Some(account) = account_flag("FINTS_PROBE_CARD_BALANCE_ACCOUNT")? {
        probe_card_balance(client, account)?;
    }
    if let Some(account) = account_flag("FINTS_PROBE_CARD_TRANSACTIONS_ACCOUNT")? {
        probe_card_transactions(client, account)?;
    }
    Ok(())
}

fn initialize_until_connected(client: &mut Client) -> ProbeResult<()> {
    let mut synchronized = false;
    for _ in 0..3 {
        let initialization = client.initialize(now());
        print_bank_responses(client, "initialize");
        match initialization? {
            Initialization::Connected => {
                print_tan_methods(client);
                print_tan_media(client);
                return Ok(());
            }
            Initialization::ChooseTanMethod => {
                print_tan_methods(client);
                choose_tan_method(client)?;
            }
            Initialization::RefreshParameters => {
                let refresh = client.refresh_parameters(now());
                print_bank_responses(client, "refresh_parameters");
                refresh?;
                print_capabilities(&client.advertised_capabilities());
                print_tan_methods(client);
                choose_tan_method(client)?;
            }
            Initialization::Challenge(continuation) => {
                print_challenge(
                    continuation.kind(),
                    continuation.challenge().text(),
                    continuation.challenge().medium_name(),
                );
                let tan = require_tan(continuation.kind())?;
                let result = client.submit_initialization_tan(*continuation, &tan, now());
                print_bank_responses(client, "submit_initialization_tan");
                match result? {
                    Initialization::Connected => return Ok(()),
                    Initialization::Challenge(_) => {
                        return Err(input_error(
                            "the probe handles one typed TAN submission per initialization",
                        ));
                    }
                    Initialization::ChooseTanMethod | Initialization::RefreshParameters => {
                        return Err(input_error(
                            "unexpected method-selection outcome after TAN submission",
                        ));
                    }
                }
            }
        }

        if !synchronized
            && enabled("FINTS_PROBE_SYNCHRONIZE")
            && client.state().system_id().is_none()
        {
            synchronize(client)?;
            synchronized = true;
        }
    }
    Err(input_error(
        "initialization did not reach Connected within the probe bound",
    ))
}

fn choose_tan_method(client: &mut Client) -> ProbeResult<()> {
    let allowed = client.allowed_tan_methods();
    let candidates = client
        .tan_methods()
        .iter()
        .filter(|method| {
            allowed
                .iter()
                .any(|allowed| allowed == method.security_function())
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(input_error(
            "no described TAN method intersects the bank's allowed method list",
        ));
    }

    let selected = select_method(&candidates)?;
    let security_function = selected.security_function().to_owned();
    let medium_required = selected.medium_name_required();
    client.select_tan_method(&security_function)?;

    if medium_required {
        println!(
            "tan_media_versions advertised={:?} selected=none",
            client.advertised_tan_media_versions()
        );
        let discovery = client.discover_tan_media(now()).map(|_| ());
        print_bank_responses(client, "discover_tan_media");
        println!(
            "tan_media_versions advertised={:?} selected={}",
            client.advertised_tan_media_versions(),
            client
                .selected_tan_media_version()
                .map(|version| version.to_string())
                .unwrap_or_else(|| "none".to_owned())
        );
        #[cfg(feature = "development-diagnostics")]
        print_tan_media_discovery_facts(client);
        discovery?;
        print_tan_media(client);
        let names = client
            .tan_media()
            .iter()
            .filter_map(|medium| medium.name().map(str::to_owned))
            .collect::<Vec<_>>();
        if names.is_empty() {
            return Err(input_error(
                "the selected method requires a named TAN medium but none was returned",
            ));
        }
        let selected_index =
            selected_index("FINTS_TAN_MEDIUM_INDEX", "TAN medium index", names.len())?;
        client.select_tan_medium(&names[selected_index])?;
    }
    Ok(())
}

fn select_method<'a>(methods: &'a [&TanMethod]) -> ProbeResult<&'a TanMethod> {
    if let Ok(requested) = env::var("FINTS_TAN_METHOD")
        && !requested.is_empty()
    {
        return methods
            .iter()
            .copied()
            .find(|method| method.security_function() == requested)
            .ok_or_else(|| input_error("FINTS_TAN_METHOD is not an offered described method"));
    }
    let index = selected_index("FINTS_TAN_METHOD_INDEX", "TAN method index", methods.len())?;
    Ok(methods[index])
}

fn synchronize(client: &mut Client) -> ProbeResult<()> {
    let result = client.synchronize(now());
    print_bank_responses(client, "synchronize");
    match result? {
        Synchronization::Complete => Ok(()),
        Synchronization::Challenge(continuation) => {
            print_challenge(
                continuation.kind(),
                continuation.challenge().text(),
                continuation.challenge().medium_name(),
            );
            let tan = require_tan(continuation.kind())?;
            let submitted = client.submit_synchronization_tan(*continuation, &tan, now());
            print_bank_responses(client, "submit_synchronization_tan");
            match submitted? {
                Synchronization::Complete => Ok(()),
                Synchronization::Challenge(_) => Err(input_error(
                    "the probe handles one typed TAN submission per synchronization",
                )),
            }
        }
    }
}

fn probe_balance(client: &mut Client, account: usize) -> ProbeResult<()> {
    let result = client.balance(account, now());
    print_bank_responses(client, "balance");
    match result? {
        BalanceRequest::Complete(_) => println!("probe operation=balance status=complete"),
        BalanceRequest::Challenge(continuation) => {
            print_challenge(
                continuation.kind(),
                continuation.challenge().text(),
                continuation.challenge().medium_name(),
            );
            let tan = require_tan(continuation.kind())?;
            let submitted = client.submit_balance_tan(*continuation, &tan, now());
            print_bank_responses(client, "submit_balance_tan");
            match submitted? {
                BalanceRequest::Complete(_) => {
                    println!("probe operation=balance status=complete")
                }
                BalanceRequest::Challenge(_) => {
                    return Err(input_error(
                        "repeated balance challenge is outside probe scope",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn probe_depot_positions(client: &mut Client, account: usize) -> ProbeResult<()> {
    let result = client.depot_positions(account, now());
    print_bank_responses(client, "depot_positions");
    #[cfg(feature = "development-diagnostics")]
    print_depot_response_facts(client);
    match result? {
        DepotPositionRequest::Complete(result) => {
            println!(
                "probe operation=depot_positions status=complete positions={} page_totals={}",
                result.positions().len(),
                result.total_values().len()
            )
        }
        DepotPositionRequest::Challenge(continuation) => {
            print_challenge(
                continuation.kind(),
                continuation.challenge().text(),
                continuation.challenge().medium_name(),
            );
            let tan = require_tan(continuation.kind())?;
            let submitted = client.submit_depot_position_tan(*continuation, &tan, now());
            print_bank_responses(client, "submit_depot_position_tan");
            #[cfg(feature = "development-diagnostics")]
            print_depot_response_facts(client);
            match submitted? {
                DepotPositionRequest::Complete(result) => {
                    println!(
                        "probe operation=depot_positions status=complete positions={} page_totals={}",
                        result.positions().len(),
                        result.total_values().len()
                    )
                }
                DepotPositionRequest::Challenge(_) => {
                    return Err(input_error(
                        "repeated depot-position challenge is outside probe scope",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn probe_depot_transactions(client: &mut Client, account: usize) -> ProbeResult<()> {
    let result = client.securities_transactions(account, None, None, now());
    print_bank_responses(client, "depot_transactions");
    #[cfg(feature = "development-diagnostics")]
    print_depot_response_facts(client);
    match result? {
        SecuritiesTransactionRequest::Complete(result) => {
            println!(
                "probe operation=depot_transactions status=complete entries={}",
                result.entries().len()
            )
        }
        SecuritiesTransactionRequest::Challenge(continuation) => {
            print_challenge(
                continuation.kind(),
                continuation.challenge().text(),
                continuation.challenge().medium_name(),
            );
            let tan = require_tan(continuation.kind())?;
            let submitted = client.submit_securities_transaction_tan(*continuation, &tan, now());
            print_bank_responses(client, "submit_securities_transaction_tan");
            #[cfg(feature = "development-diagnostics")]
            print_depot_response_facts(client);
            match submitted? {
                SecuritiesTransactionRequest::Complete(result) => {
                    println!(
                        "probe operation=depot_transactions status=complete entries={}",
                        result.entries().len()
                    )
                }
                SecuritiesTransactionRequest::Challenge(_) => {
                    return Err(input_error(
                        "repeated depot-transaction challenge is outside probe scope",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn probe_card_balance(client: &mut Client, account: usize) -> ProbeResult<()> {
    let result = client.credit_card_balance(account, now());
    print_bank_responses(client, "credit_card_balance");
    match result? {
        CreditCardBalanceRequest::Complete(_) => {
            println!("probe operation=credit_card_balance status=complete")
        }
        CreditCardBalanceRequest::Challenge(continuation) => {
            print_challenge(
                continuation.kind(),
                continuation.challenge().text(),
                continuation.challenge().medium_name(),
            );
            let tan = require_tan(continuation.kind())?;
            let submitted = client.submit_credit_card_balance_tan(*continuation, &tan, now());
            print_bank_responses(client, "submit_credit_card_balance_tan");
            match submitted? {
                CreditCardBalanceRequest::Complete(_) => {
                    println!("probe operation=credit_card_balance status=complete")
                }
                CreditCardBalanceRequest::Challenge(_) => {
                    return Err(input_error(
                        "repeated credit-card balance challenge is outside probe scope",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn probe_card_transactions(client: &mut Client, account: usize) -> ProbeResult<()> {
    let result = client.credit_card_transactions(account, None, None, now());
    print_bank_responses(client, "credit_card_transactions");
    match result? {
        CreditCardTransactionRequest::Complete(_) => {
            println!("probe operation=credit_card_transactions status=complete")
        }
        CreditCardTransactionRequest::Challenge(continuation) => {
            print_challenge(
                continuation.kind(),
                continuation.challenge().text(),
                continuation.challenge().medium_name(),
            );
            let tan = require_tan(continuation.kind())?;
            let submitted = client.submit_credit_card_transaction_tan(*continuation, &tan, now());
            print_bank_responses(client, "submit_credit_card_transaction_tan");
            match submitted? {
                CreditCardTransactionRequest::Complete(_) => {
                    println!("probe operation=credit_card_transactions status=complete")
                }
                CreditCardTransactionRequest::Challenge(_) => {
                    return Err(input_error(
                        "repeated credit-card transaction challenge is outside probe scope",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn print_capabilities(snapshot: &AdvertisedCapabilitySnapshot) {
    print_operation("balance", snapshot.balance());
    print_operation("cash_camt", snapshot.camt_cash_transactions());
    print_operation("cash_mt940", snapshot.mt940_cash_transactions());
    print_operation("depot_positions", snapshot.depot_positions());
    print_operation("depot_transactions", snapshot.depot_transactions());
    print_operation(
        "credit_card_transactions",
        snapshot.credit_card_transactions(),
    );
    print_operation("credit_card_balance", snapshot.credit_card_balance());
    for segment in snapshot.parameter_segments() {
        println!(
            "parameter_segment code={} version={}",
            segment.code(),
            segment.version()
        );
    }
}

fn print_operation(name: &str, capability: &OperationCapabilitySnapshot) {
    if capability.advertised_versions().is_empty() {
        println!(
            "capability operation={name} advertised={} version=none supported=false tan_required={}",
            capability.advertised(),
            tan_requirement(capability.tan_required())
        );
    } else {
        for version in capability.advertised_versions() {
            println!(
                "capability operation={name} advertised=true version={version} supported={} tan_required={}",
                capability.supports_version(*version),
                tan_requirement(capability.tan_required())
            );
        }
    }
    for descriptor in capability.descriptors() {
        println!("capability operation={name} descriptor={descriptor}");
    }
}

fn print_tan_methods(client: &Client) {
    for (index, method) in client.tan_methods().iter().enumerate() {
        let allowed = client
            .allowed_tan_methods()
            .iter()
            .any(|allowed| allowed == method.security_function());
        println!(
            "tan_method index={index} function={} hktan={} process={:?} allowed={} technical_id={} display_name={} medium_name_required={}",
            method.security_function(),
            method.hktan_version(),
            method.process(),
            allowed,
            method.technical_id(),
            method.display_name(),
            method.medium_name_required()
        );
        #[cfg(feature = "development-diagnostics")]
        if let Some(requirement) = method.development_medium_requirement() {
            println!(
                "tan_method_hitans index={index} hktan={} requirement_code={} requirement_field={} requirement_index={} active_count={} active_count_field={} active_count_index={} medium_name_required={}",
                requirement.hktan_version(),
                requirement.requirement_code(),
                requirement.requirement_field_number(),
                requirement.requirement_component_index(),
                requirement
                    .active_media_count()
                    .map(|count| count.to_string())
                    .unwrap_or_else(|| "none".to_owned()),
                requirement.active_media_count_field_number(),
                requirement.active_media_count_component_index(),
                requirement.medium_name_required(),
            );
        }
    }
}

fn print_tan_media(client: &Client) {
    for (index, medium) in client.tan_media().iter().enumerate() {
        println!(
            "tan_medium index={index} class={:?} status={:?} function={} name_present={} masked_phone_present={}",
            medium.class(),
            medium.status(),
            medium.security_function().unwrap_or("none"),
            medium.name().is_some(),
            medium.masked_phone().is_some()
        );
    }
}

#[cfg(feature = "development-diagnostics")]
fn print_tan_media_discovery_facts(client: &Client) {
    let Some(facts) = client.development_tan_media_discovery() else {
        println!("tan_media_diagnostics available=false");
        return;
    };
    if let Some(requirement) = facts.hitans_requirement() {
        println!(
            "tan_media_hitans hktan={} requirement_code={} requirement_field={} requirement_index={} active_count={} active_count_field={} active_count_index={} medium_name_required={}",
            requirement.hktan_version(),
            requirement.requirement_code(),
            requirement.requirement_field_number(),
            requirement.requirement_component_index(),
            requirement
                .active_media_count()
                .map(|count| count.to_string())
                .unwrap_or_else(|| "none".to_owned()),
            requirement.active_media_count_field_number(),
            requirement.active_media_count_component_index(),
            requirement.medium_name_required(),
        );
    }
    if let Some(request) = facts.hktab_request() {
        println!(
            "tan_media_hktab version={} medium_type={} medium_class={} medium_name_field_present={}",
            request.version(),
            request.medium_type(),
            request
                .medium_class()
                .map(|class| format!("{class:?}"))
                .unwrap_or_else(|| "none".to_owned()),
            request.medium_name_field_present(),
        );
    }
    println!(
        "tan_media_initialization hktan_medium_name_supplied={} tan_usage_option={}",
        facts
            .initialization_hktan_medium_name_supplied()
            .map(|supplied| supplied.to_string())
            .unwrap_or_else(|| "unknown".to_owned()),
        facts
            .tan_usage_option()
            .map(|option| option.to_string())
            .unwrap_or_else(|| "unknown".to_owned())
    );
    for (index, medium) in facts.returned_media().iter().enumerate() {
        println!(
            "tan_media_hitab index={index} class={:?} status={:?} name_present={} card_number_present={} card_sequence_present={}",
            medium.class(),
            medium.status(),
            medium.name_present(),
            medium.card_number_present(),
            medium.card_sequence_present(),
        );
    }
    for (index, shape) in facts.returned_medium_shapes().iter().enumerate() {
        println!(
            "tan_media_hitab_shape index={index} component_count={} occupied_components={:?}",
            shape.component_count(),
            shape.occupied_components()
        );
    }
}

#[cfg(feature = "development-diagnostics")]
fn print_depot_response_facts(client: &Client) {
    let Some(facts) = client.development_depot_response() else {
        println!("depot_diagnostics available=false");
        return;
    };
    println!("depot_document kind={:?}", facts.document_kind());
    for (index, block) in facts.block_inventory().iter().enumerate() {
        println!(
            "depot_block index={index} kind={:?} depth={} occurrence={}",
            block.kind(),
            block.depth(),
            block.occurrence()
        );
    }
    for (index, tag) in facts.tag_inventory().iter().enumerate() {
        println!(
            "depot_tag index={index} kind={:?} depth={} occurrence={}",
            tag.kind(),
            tag.depth(),
            tag.occurrence()
        );
    }
    for (index, price) in facts.price_shapes().iter().enumerate() {
        println!(
            "depot_price_shape index={index} tag={:?} qualifier={:?} unit={:?} qualifier_present={} unit_present={} currency_present={} price_present={}",
            price.tag(),
            price.qualifier(),
            price.unit(),
            price.qualifier_present(),
            price.unit_present(),
            price.currency_present(),
            price.price_present()
        );
    }
    for (index, position) in facts.positions().iter().enumerate() {
        println!(
            "depot_position index={index} isin_present={} wkn_present={} name_present={} quantity_present={} price_present={} market_value_present={} cost_basis_present={}",
            position.isin_present(),
            position.wkn_present(),
            position.name_present(),
            position.quantity_present(),
            position.price_present(),
            position.market_value_present(),
            position.cost_basis_present()
        );
    }
    for (index, entry) in facts.transactions().iter().enumerate() {
        println!(
            "depot_transaction index={index} isin_present={} wkn_present={} name_present={} reference_present={} quantity_present={} price_present={} amount_present={} accrued_interest_present={} transaction_kind_present={} movement_present={} effective_date_present={} value_date_present={} reversal_present={} free_text_present={}",
            entry.isin_present(),
            entry.wkn_present(),
            entry.name_present(),
            entry.reference_present(),
            entry.quantity_present(),
            entry.price_present(),
            entry.amount_present(),
            entry.accrued_interest_present(),
            entry.transaction_kind_present(),
            entry.movement_present(),
            entry.effective_date_present(),
            entry.value_date_present(),
            entry.reversal_present(),
            entry.free_text_present()
        );
    }
}

fn print_bank_responses(client: &Client, operation: &str) {
    print_responses(client.last_responses(), operation);
}

fn print_responses(responses: &[BankResponse], operation: &str) {
    for response in responses {
        println!(
            "bank_response operation={operation} code={:04} class={:?} segment={} data_element={} text={} parameters={}",
            response.code(),
            response.class(),
            response
                .segment_number()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_owned()),
            response.data_element_reference().unwrap_or("none"),
            response.text(),
            response.parameters().join("|")
        );
    }
}

fn print_challenge(kind: ContinuationKind, text: Option<&str>, medium: Option<&str>) {
    println!(
        "challenge kind={kind:?} medium={} text={}",
        medium.unwrap_or("none"),
        text.unwrap_or("none")
    );
}

fn require_tan(continuation_kind: ContinuationKind) -> ProbeResult<Tan> {
    if continuation_kind != ContinuationKind::Tan {
        return Err(input_error(
            "decoupled polling is outside the bounded live probe; the dialog was terminated",
        ));
    }
    let value = stdin_input("TAN (empty input cancels; stdin may be echoed)")?;
    let Some(value) = value else {
        return Err(input_error("TAN entry cancelled"));
    };
    Ok(Tan::new(value)?)
}

fn raw_stdout_trace() -> TraceSink {
    Box::new(|event: TraceEvent<'_>| {
        let mut hex = String::with_capacity(event.payload().len().saturating_mul(2));
        for byte in event.payload() {
            let _ = write!(hex, "{byte:02x}");
        }
        let text = decode_latin1(event.payload())
            .chars()
            .flat_map(char::escape_default)
            .collect::<String>();
        println!(
            "trace exchange={} direction={} hex={hex}",
            event.exchange_index(),
            match event.direction() {
                TraceDirection::Outgoing => "outgoing",
                TraceDirection::Incoming => "incoming",
            }
        );
        println!(
            "trace exchange={} direction={} text_escaped={text}",
            event.exchange_index(),
            match event.direction() {
                TraceDirection::Outgoing => "outgoing",
                TraceDirection::Incoming => "incoming",
            }
        );
    })
}

fn required_input(variable: &str, prompt: &str) -> ProbeResult<String> {
    optional_input(variable, prompt)?
        .ok_or_else(|| input_error(&format!("{variable} must not be empty")))
}

fn optional_input(variable: &str, prompt: &str) -> ProbeResult<Option<String>> {
    if let Ok(value) = env::var(variable) {
        return Ok((!value.is_empty()).then_some(value));
    }
    stdin_input(prompt)
}

fn stdin_input(prompt: &str) -> ProbeResult<Option<String>> {
    print!("{prompt}: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    let value = value.trim_end_matches(['\r', '\n']).to_owned();
    Ok((!value.is_empty()).then_some(value))
}

fn selected_index(variable: &str, prompt: &str, count: usize) -> ProbeResult<usize> {
    let value = required_input(variable, prompt)?;
    let index = value
        .parse::<usize>()
        .map_err(|_| input_error(&format!("{variable} must be a zero-based index")))?;
    if index >= count {
        return Err(input_error(&format!(
            "{variable} is outside the offered zero-based range"
        )));
    }
    Ok(index)
}

fn account_flag(variable: &str) -> ProbeResult<Option<usize>> {
    let Ok(value) = env::var(variable) else {
        return Ok(None);
    };
    let index = value
        .parse::<usize>()
        .map_err(|_| input_error(&format!("{variable} must be a zero-based account index")))?;
    Ok(Some(index))
}

fn enabled(variable: &str) -> bool {
    enabled_value(env::var(variable).ok().as_deref())
}

fn enabled_value(value: Option<&str>) -> bool {
    value == Some("1")
}

fn tan_requirement(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    }
}

fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

fn input_error(message: &str) -> Box<dyn Error> {
    Box::new(io::Error::new(
        io::ErrorKind::InvalidInput,
        message.to_owned(),
    ))
}

#[cfg(test)]
mod tests {
    use super::enabled_value;

    #[test]
    fn opt_in_gates_accept_only_the_exact_value_one() {
        assert!(enabled_value(Some("1")));
        for value in [None, Some(""), Some("0"), Some("true"), Some("yes")] {
            assert!(!enabled_value(value));
        }
    }
}
