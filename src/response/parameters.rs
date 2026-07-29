use crate::{
    error::Error,
    model::{
        Account, CamtCapability, CreditCardCapability, InstituteState, OperationPermission,
        ReusableState, TanMedium, TanMediumClass, TanMediumStatus, TanMethod, TanProcess,
    },
    wire::{Segment, Value},
};

use super::{component, optional_component};

pub(super) fn apply(
    segments: &[Segment],
    state: &mut ReusableState,
) -> Result<Option<Vec<Account>>, Error> {
    let mut received_accounts = Vec::new();
    let mut received_methods = Vec::new();
    let mut received_balance_versions = Vec::new();
    let mut balance_capability_advertised = false;
    let mut received_camt_descriptors = Vec::new();
    let mut received_legacy_transaction_versions = Vec::new();
    let mut transaction_capability_advertised = false;
    let mut received_upd_version = None;
    let mut received_bpd_version = None;
    let mut balance_requires_tan = None;
    let mut camt_requires_tan = None;
    let mut legacy_transactions_require_tan = None;
    let mut depot_positions_advertised = false;
    let mut depot_positions_supported = false;
    let mut depot_positions_requires_tan = None;
    let mut securities_transactions_advertised = false;
    let mut securities_transactions_supported = false;
    let mut securities_transactions_requires_tan = None;
    let mut credit_card_transactions_advertised = false;
    let mut credit_card_transactions = None;
    let mut credit_card_transactions_requires_tan = None;
    let mut credit_card_balance_advertised = false;
    let mut credit_card_balance_account_required = None;
    let mut credit_card_balance_requires_tan = None;

    for segment in segments {
        let header = segment.header().ok_or(Error::InvalidResponse {
            structure: "segment header",
        })?;
        match header.code {
            b"HIBPA" => {
                require_version(header.version, 3, "HIBPA")?;
                let version = parse_element_u16(segment, 1, "BPD version")?;
                if version == 0 || version > 999 {
                    return Err(Error::InvalidValue {
                        field: "BPD version",
                    });
                }
                received_bpd_version = Some(version);
            }
            b"HIUPA" => {
                require_version(header.version, 4, "HIUPA")?;
                let version = parse_element_u16(segment, 2, "UPD version")?;
                if version > 999 {
                    return Err(Error::InvalidValue {
                        field: "UPD version",
                    });
                }
                received_upd_version = Some(version);
            }
            b"HIUPD" => {
                require_version(header.version, 6, "HIUPD")?;
                if let Some(account) = parse_account(segment)? {
                    received_accounts.push(account);
                }
            }
            b"HISALS" => {
                balance_capability_advertised = true;
                if (6..=8).contains(&header.version) {
                    received_balance_versions.push(header.version);
                }
            }
            b"HICAZS" => {
                transaction_capability_advertised = true;
                if header.version == 1 {
                    received_camt_descriptors.extend(parse_camt_descriptors(segment)?);
                }
            }
            b"HIKAZS" => {
                transaction_capability_advertised = true;
                if (6..=7).contains(&header.version) {
                    require_transaction_parameters(segment)?;
                    received_legacy_transaction_versions.push(header.version);
                }
            }
            b"HIWPDS" => {
                depot_positions_advertised = true;
                if header.version == 6 {
                    if depot_positions_supported {
                        return Err(Error::InvalidResponse {
                            structure: "duplicate HIWPDS version 6",
                        });
                    }
                    require_depot_position_parameters(segment)?;
                    depot_positions_supported = true;
                }
            }
            b"HIWDUS" => {
                securities_transactions_advertised = true;
                if header.version == 5 {
                    if securities_transactions_supported {
                        return Err(Error::InvalidResponse {
                            structure: "duplicate HIWDUS version 5",
                        });
                    }
                    require_depot_transaction_parameters(segment)?;
                    securities_transactions_supported = true;
                }
            }
            b"HIKKUS" => {
                credit_card_transactions_advertised = true;
                if header.version == 1 {
                    if credit_card_transactions.is_some() {
                        return Err(Error::InvalidResponse {
                            structure: "duplicate HIKKUS version 1",
                        });
                    }
                    credit_card_transactions = Some(parse_credit_card_parameters(segment)?);
                }
            }
            b"HIKKSS" => {
                credit_card_balance_advertised = true;
                if header.version == 1 {
                    if credit_card_balance_account_required.is_some() {
                        return Err(Error::InvalidResponse {
                            structure: "duplicate HIKKSS version 1",
                        });
                    }
                    credit_card_balance_account_required =
                        Some(parse_credit_card_balance_parameters(segment)?);
                }
            }
            b"HIPINS" => {
                require_version(header.version, 1, "HIPINS")?;
                balance_requires_tan = parse_tan_requirement(segment, "HKSAL")?;
                camt_requires_tan = parse_tan_requirement(segment, "HKCAZ")?;
                legacy_transactions_require_tan = parse_tan_requirement(segment, "HKKAZ")?;
                depot_positions_requires_tan = parse_tan_requirement(segment, "HKWPD")?;
                securities_transactions_requires_tan = parse_tan_requirement(segment, "HKWDU")?;
                credit_card_transactions_requires_tan = parse_tan_requirement(segment, "HKKKU")?;
                credit_card_balance_requires_tan = parse_tan_requirement(segment, "HKKKS")?;
            }
            b"HITANS" if (6..=7).contains(&header.version) => {
                received_methods.extend(parse_tan_methods(segment, header.version)?);
            }
            _ => {}
        }
    }

    if let Some(version) = received_bpd_version {
        state.bpd_version = version;
        received_balance_versions.sort_unstable_by(|left, right| right.cmp(left));
        received_balance_versions.dedup();
        received_legacy_transaction_versions.sort_unstable_by(|left, right| right.cmp(left));
        received_legacy_transaction_versions.dedup();
        received_camt_descriptors.sort();
        received_camt_descriptors.dedup();
        received_methods.sort_by(|left, right| {
            left.security_function
                .cmp(&right.security_function)
                .then_with(|| right.hktan_version.cmp(&left.hktan_version))
        });
        received_methods.dedup_by(|left, right| left.security_function == right.security_function);
        state.balance_versions = received_balance_versions;
        state.balance_capability_advertised = balance_capability_advertised;
        state.balance_requires_tan = balance_requires_tan;
        state.transaction_capability_advertised = transaction_capability_advertised;
        state.camt_capability = received_camt_descriptors
            .into_iter()
            .find(|descriptor| descriptor == "urn:iso:std:iso:20022:tech:xsd:camt.052.001.08")
            .map(|descriptor| CamtCapability { descriptor });
        state.legacy_transaction_versions = received_legacy_transaction_versions;
        state.camt_requires_tan = camt_requires_tan;
        state.legacy_transactions_require_tan = legacy_transactions_require_tan;
        state.depot_positions_advertised = depot_positions_advertised;
        state.depot_positions_supported = depot_positions_supported;
        state.depot_positions_requires_tan = depot_positions_requires_tan;
        state.securities_transactions_advertised = securities_transactions_advertised;
        state.securities_transactions_supported = securities_transactions_supported;
        state.securities_transactions_requires_tan = securities_transactions_requires_tan;
        state.credit_card_transactions_advertised = credit_card_transactions_advertised;
        state.credit_card_transactions = credit_card_transactions;
        state.credit_card_transactions_requires_tan = credit_card_transactions_requires_tan;
        state.credit_card_balance_advertised = credit_card_balance_advertised;
        state.credit_card_balance_account_required = credit_card_balance_account_required;
        state.credit_card_balance_requires_tan = credit_card_balance_requires_tan;
        state.tan_methods = received_methods;
    }

    let mut transient_accounts = None;
    if let Some(version) = received_upd_version {
        state.upd_version = version;
        if version > 0 {
            state.accounts = received_accounts;
        } else {
            transient_accounts = Some(received_accounts);
        }
    }
    Ok(transient_accounts)
}

pub(super) fn bpd_institute(segments: &[Segment]) -> Result<Option<InstituteState>, Error> {
    let Some(segment) = segments.iter().find(|segment| {
        segment
            .header()
            .is_some_and(|header| header.code == b"HIBPA")
    }) else {
        return Ok(None);
    };
    require_version(
        segment.header().expect("header checked").version,
        3,
        "HIBPA",
    )?;
    let components = segment
        .elements()
        .get(2)
        .ok_or(Error::MissingValue {
            field: "BPD institute identity",
        })?
        .components();
    Ok(Some(InstituteState {
        country_code: component(components, 0, "BPD country code")?,
        institute_code: component(components, 1, "BPD institute code")?,
    }))
}

pub(super) fn system_id(segments: &[Segment]) -> Result<Option<String>, Error> {
    let Some(segment) = segments.iter().find(|segment| {
        segment
            .header()
            .is_some_and(|header| header.code == b"HISYN")
    }) else {
        return Ok(None);
    };
    require_version(
        segment.header().expect("header checked").version,
        4,
        "HISYN",
    )?;
    Ok(Some(single_text(segment, 1, "assigned system ID")?))
}

pub(super) fn tan_media(segments: &[Segment]) -> Result<Vec<TanMedium>, Error> {
    let Some(segment) = segments.iter().find(|segment| {
        segment
            .header()
            .is_some_and(|header| header.code == b"HITAB")
    }) else {
        return Ok(Vec::new());
    };
    require_version(
        segment.header().expect("header checked").version,
        5,
        "HITAB",
    )?;
    let mut media = Vec::new();
    for element in segment.elements().iter().skip(2) {
        let components = element.components();
        let class = match component(components, 0, "TAN medium class")?.as_str() {
            "A" => TanMediumClass::All,
            "G" => TanMediumClass::Generator,
            "L" => TanMediumClass::List,
            "M" => TanMediumClass::Mobile,
            "S" => TanMediumClass::Secoder,
            "B" => TanMediumClass::Bilateral,
            _ => {
                return Err(Error::InvalidValue {
                    field: "TAN medium class",
                });
            }
        };
        let status = match component(components, 1, "TAN medium status")?.as_str() {
            "1" => TanMediumStatus::Active,
            "2" => TanMediumStatus::Available,
            "3" => TanMediumStatus::FollowUpActive,
            "4" => TanMediumStatus::FollowUpAvailable,
            _ => {
                return Err(Error::InvalidValue {
                    field: "TAN medium status",
                });
            }
        };
        match class {
            TanMediumClass::Generator if optional_component(components, 3).is_none() => {
                return Err(Error::MissingValue {
                    field: "TAN generator card number",
                });
            }
            TanMediumClass::List if optional_component(components, 9).is_none() => {
                return Err(Error::MissingValue {
                    field: "TAN list number",
                });
            }
            TanMediumClass::Mobile
                if optional_component(components, 10).is_none()
                    || (optional_component(components, 11).is_none()
                        && optional_component(components, 12).is_none()) =>
            {
                return Err(Error::MissingValue {
                    field: "mobile TAN medium identity",
                });
            }
            TanMediumClass::Bilateral => {
                let security_function =
                    optional_component(components, 2).ok_or(Error::MissingValue {
                        field: "bilateral TAN security function",
                    })?;
                if security_function.len() != 3
                    || !security_function.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(Error::InvalidValue {
                        field: "bilateral TAN security function",
                    });
                }
            }
            _ => {}
        }
        media.push(TanMedium {
            class,
            status,
            security_function: optional_component(components, 2),
            name: optional_component(components, 10),
            masked_phone: optional_component(components, 11),
        });
    }
    Ok(media)
}

fn parse_account(segment: &Segment) -> Result<Option<Account>, Error> {
    let elements = segment.elements();
    let national = elements
        .get(1)
        .ok_or(Error::MissingValue {
            field: "UPD account",
        })?
        .components();
    let account_number = optional_component(national, 0);
    let subaccount = optional_component(national, 1);
    let institute = match (
        optional_component(national, 2),
        optional_component(national, 3),
    ) {
        (Some(country_code), Some(institute_code)) => Some(InstituteState {
            country_code,
            institute_code,
        }),
        (None, None) => None,
        _ => {
            return Err(Error::InvalidResponse {
                structure: "UPD institute identity",
            });
        }
    };
    let mut iban = optional_single_text(elements, 2);
    if account_number.is_none() && iban.is_none() {
        // Formals E.3 permits HIUPD records for non-account-bound operations.
        return Ok(None);
    }
    if iban
        .as_ref()
        .is_some_and(|value| value.chars().count() == 35)
    {
        // Formals HIUPD 6 correction: tolerate the original erroneous 35-character
        // maximum by discarding exactly the final character.
        iban.as_mut().expect("IBAN checked").pop();
    }

    let mut allowed_operations = Vec::new();
    for element in elements.iter().skip(10) {
        let components = element.components();
        let Some(code) = optional_component(components, 0) else {
            continue;
        };
        if code.is_empty()
            || code.len() > 6
            || !code
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
        {
            continue;
        }
        let Some(required_signatures) =
            optional_component(components, 1).and_then(|value| value.parse::<u8>().ok())
        else {
            continue;
        };
        allowed_operations.push(OperationPermission {
            code,
            required_signatures,
        });
    }

    Ok(Some(Account {
        iban,
        bic: None,
        account_number,
        subaccount,
        institute,
        currency: optional_single_text(elements, 5),
        account_type: optional_single_text(elements, 4)
            .map(|value| {
                value.parse().map_err(|_| Error::InvalidValue {
                    field: "UPD account type",
                })
            })
            .transpose()?,
        owner_name_1: optional_single_text(elements, 6),
        owner_name_2: optional_single_text(elements, 7),
        product_name: optional_single_text(elements, 8),
        allowed_operations,
    }))
}

fn parse_tan_requirement(segment: &Segment, operation: &str) -> Result<Option<bool>, Error> {
    let Some(parameters) = segment.elements().get(4) else {
        return Err(Error::MissingValue {
            field: "HIPINS parameters",
        });
    };
    let components = parameters.components();
    for pair in components.get(5..).unwrap_or_default().chunks_exact(2) {
        if optional_component(pair, 0).as_deref() == Some(operation) {
            return match optional_component(pair, 1).as_deref() {
                Some("J") => Ok(Some(true)),
                Some("N") => Ok(Some(false)),
                _ => Err(Error::InvalidValue {
                    field: "operation TAN requirement",
                }),
            };
        }
    }
    Ok(None)
}

fn parse_credit_card_parameters(segment: &Segment) -> Result<CreditCardCapability, Error> {
    // G112 / CR 538 C.12.1 HIKKUS 1: storage period, maximum-entry input,
    // date-range input, and conditional account binding.
    let components = segment
        .elements()
        .get(4)
        .ok_or(Error::MissingValue {
            field: "HIKKUS parameters",
        })?
        .components();
    let storage_days = component(components, 0, "HIKKUS storage period")?
        .parse::<u16>()
        .map_err(|_| Error::InvalidValue {
            field: "HIKKUS storage period",
        })?;
    if storage_days == 0 {
        return Err(Error::InvalidValue {
            field: "HIKKUS storage period",
        });
    }
    Ok(CreditCardCapability {
        entry_count_allowed: yn(components, 1, "HIKKUS maximum-entry input")?,
        date_range_allowed: yn(components, 2, "HIKKUS date-range input")?,
        account_required: yn(components, 3, "HIKKUS account binding")?,
    })
}

fn require_depot_position_parameters(segment: &Segment) -> Result<(), Error> {
    // Messages 2022 Data Dictionary, Parameter Depotaufstellung version 2.
    let components = segment
        .elements()
        .get(4)
        .ok_or(Error::MissingValue {
            field: "HIWPDS parameters",
        })?
        .components();
    yn(components, 0, "HIWPDS maximum-entry input")?;
    yn(components, 1, "HIWPDS currency selection")?;
    yn(components, 2, "HIWPDS price-quality selection")?;
    Ok(())
}

fn require_depot_transaction_parameters(segment: &Segment) -> Result<(), Error> {
    // Messages 2022 Data Dictionary, Parameter Depotumsätze version 1.
    let components = segment
        .elements()
        .get(4)
        .ok_or(Error::MissingValue {
            field: "HIWDUS parameters",
        })?
        .components();
    let storage_days = component(components, 0, "HIWDUS storage period")?
        .parse::<u16>()
        .map_err(|_| Error::InvalidValue {
            field: "HIWDUS storage period",
        })?;
    if storage_days == 0 {
        return Err(Error::InvalidValue {
            field: "HIWDUS storage period",
        });
    }
    Ok(())
}

fn parse_credit_card_balance_parameters(segment: &Segment) -> Result<bool, Error> {
    // G112 / CR 538 C.12.2 HIKKSS 1.
    let components = segment
        .elements()
        .get(4)
        .ok_or(Error::MissingValue {
            field: "HIKKSS parameters",
        })?
        .components();
    yn(components, 0, "HIKKSS account binding")
}

fn yn(components: &[Value], index: usize, field: &'static str) -> Result<bool, Error> {
    match component(components, index, field)?.as_str() {
        "J" => Ok(true),
        "N" => Ok(false),
        _ => Err(Error::InvalidValue { field }),
    }
}

fn parse_camt_descriptors(segment: &Segment) -> Result<Vec<String>, Error> {
    let components = segment
        .elements()
        .get(4)
        .ok_or(Error::MissingValue {
            field: "HICAZS parameters",
        })?
        .components();
    if components.len() < 4 {
        return Err(Error::InvalidResponse {
            structure: "HICAZS parameters",
        });
    }
    component(components, 0, "HICAZS storage period")?
        .parse::<u16>()
        .map_err(|_| Error::InvalidValue {
            field: "HICAZS storage period",
        })?;
    for index in 1..=2 {
        if !matches!(
            component(components, index, "HICAZS yes/no parameter")?.as_str(),
            "J" | "N"
        ) {
            return Err(Error::InvalidValue {
                field: "HICAZS yes/no parameter",
            });
        }
    }
    components
        .iter()
        .skip(3)
        .map(|value| {
            value
                .as_text()
                .filter(|value| !value.is_empty() && value.len() <= 256)
                .map(|value| value.into_owned())
                .ok_or(Error::InvalidValue {
                    field: "camt descriptor",
                })
        })
        .collect()
}

fn require_transaction_parameters(segment: &Segment) -> Result<(), Error> {
    let components = segment
        .elements()
        .get(4)
        .ok_or(Error::MissingValue {
            field: "HIKAZS parameters",
        })?
        .components();
    if components.len() < 3 {
        return Err(Error::InvalidResponse {
            structure: "HIKAZS parameters",
        });
    }
    Ok(())
}

fn parse_tan_methods(segment: &Segment, version: u16) -> Result<Vec<TanMethod>, Error> {
    let parameters = segment
        .elements()
        .get(4)
        .ok_or(Error::MissingValue {
            field: "HITANS parameters",
        })?
        .components();
    if parameters.len() < 3 {
        return Err(Error::InvalidResponse {
            structure: "HITANS parameters",
        });
    }
    let width = if version == 6 { 21 } else { 26 };
    let method_values = &parameters[3..];
    if method_values.is_empty() {
        return Err(Error::InvalidResponse {
            structure: "HITANS method parameters",
        });
    }
    let mut blocks = method_values.chunks_exact(width);
    let mut methods = blocks
        .by_ref()
        .map(|components| parse_tan_method(components, version))
        .collect::<Result<Vec<_>, _>>()?;
    let trailing = blocks.remainder();
    if !trailing.is_empty() {
        methods.push(parse_tan_method(trailing, version)?);
    }
    if methods.len() > 98 {
        return Err(Error::InvalidResponse {
            structure: "too many HITANS methods",
        });
    }
    Ok(methods)
}

fn parse_tan_method(components: &[Value], version: u16) -> Result<TanMethod, Error> {
    // Fields through "Antwort HHD_UC erforderlich" are positional and mandatory.
    // Only trailing optional fields of the last repeated method may be cut.
    if components.len() < 20 {
        return Err(Error::InvalidResponse {
            structure: "cut HITANS method parameters",
        });
    }
    let security_function = component(components, 0, "TAN security function")?;
    let numeric_function: u16 = security_function.parse().map_err(|_| Error::InvalidValue {
        field: "TAN security function",
    })?;
    if !(900..=997).contains(&numeric_function) {
        return Err(Error::InvalidValue {
            field: "TAN security function",
        });
    }
    let process_variant = component(components, 1, "TAN process variant")?;
    let dk_method = optional_component(components, 3);
    let process =
        if version == 7 && matches!(dk_method.as_deref(), Some("Decoupled" | "DecoupledPush")) {
            if process_variant != "2" {
                return Err(Error::InvalidValue {
                    field: "decoupled process variant",
                });
            }
            TanProcess::Decoupled
        } else {
            match process_variant.as_str() {
                "1" => TanProcess::ProcessVariantOne,
                "2" => TanProcess::ProcessVariantTwo,
                _ => {
                    return Err(Error::InvalidValue {
                        field: "TAN process variant",
                    });
                }
            }
        };
    if process == TanProcess::Decoupled && components.len() < 24 {
        return Err(Error::InvalidResponse {
            structure: "cut decoupled HITANS method parameters",
        });
    }
    let medium_name_required = match component(components, 18, "TAN medium requirement")?.as_str() {
        "0" | "1" => false,
        "2" => true,
        _ => {
            return Err(Error::InvalidValue {
                field: "TAN medium requirement",
            });
        }
    };

    Ok(TanMethod {
        security_function,
        hktan_version: version,
        process,
        technical_id: component(components, 2, "TAN method technical ID")?,
        display_name: component(components, 5, "TAN method name")?,
        dk_method,
        max_tan_length: optional_component(components, 6)
            .map(|value| {
                value.parse().map_err(|_| Error::InvalidValue {
                    field: "maximum TAN length",
                })
            })
            .transpose()?,
        tan_format: optional_component(components, 7),
        medium_name_required,
        hhd_response_required: optional_yes(components, 19)?,
        max_decoupled_polls: (version == 7)
            .then(|| optional_u16(components, 21, "maximum decoupled polls"))
            .transpose()?
            .flatten()
            .and_then(|value| (value != 0).then_some(value)),
        first_poll_delay_seconds: (version == 7)
            .then(|| optional_u16(components, 22, "first decoupled poll delay"))
            .transpose()?
            .flatten(),
        next_poll_delay_seconds: (version == 7)
            .then(|| optional_u16(components, 23, "next decoupled poll delay"))
            .transpose()?
            .flatten(),
        manual_polling_allowed: version == 7 && optional_yes(components, 24)?,
        automatic_polling_allowed: version == 7 && optional_yes(components, 25)?,
    })
}

fn optional_yes(components: &[Value], index: usize) -> Result<bool, Error> {
    match optional_component(components, index).as_deref() {
        None | Some("N") => Ok(false),
        Some("J") => Ok(true),
        _ => Err(Error::InvalidValue {
            field: "yes/no parameter",
        }),
    }
}

fn optional_u16(
    components: &[Value],
    index: usize,
    field: &'static str,
) -> Result<Option<u16>, Error> {
    optional_component(components, index)
        .map(|value| value.parse().map_err(|_| Error::InvalidValue { field }))
        .transpose()
}

fn parse_element_u16(segment: &Segment, index: usize, field: &'static str) -> Result<u16, Error> {
    single_text(segment, index, field)?
        .parse()
        .map_err(|_| Error::InvalidValue { field })
}

fn single_text(segment: &Segment, index: usize, field: &'static str) -> Result<String, Error> {
    segment
        .elements()
        .get(index)
        .map(|element| component(element.components(), 0, field))
        .transpose()?
        .ok_or(Error::MissingValue { field })
}

fn optional_single_text(elements: &[crate::wire::Element], index: usize) -> Option<String> {
    elements
        .get(index)
        .and_then(|element| optional_component(element.components(), 0))
}

fn require_version(actual: u16, expected: u16, code: &'static str) -> Result<(), Error> {
    if actual == expected {
        Ok(())
    } else {
        Err(Error::UnsupportedSegment {
            code,
            version: actual,
        })
    }
}
