use crate::{
    error::Error,
    model::{
        Account, InstituteState, OperationPermission, ReusableState, TanMedium, TanMediumClass,
        TanMediumStatus, TanMethod, TanProcess,
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
    let mut received_upd_version = None;
    let mut received_bpd_version = None;
    let mut balance_requires_tan = None;

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
                received_accounts.push(parse_account(segment)?);
            }
            b"HISALS" if (6..=8).contains(&header.version) => {
                received_balance_versions.push(header.version);
            }
            b"HIPINS" => {
                require_version(header.version, 1, "HIPINS")?;
                balance_requires_tan = parse_balance_tan_requirement(segment)?;
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
        received_methods.sort_by(|left, right| {
            left.security_function
                .cmp(&right.security_function)
                .then_with(|| right.hktan_version.cmp(&left.hktan_version))
        });
        received_methods.dedup_by(|left, right| left.security_function == right.security_function);
        state.balance_versions = received_balance_versions;
        state.balance_requires_tan = balance_requires_tan;
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

fn parse_account(segment: &Segment) -> Result<Account, Error> {
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
    let iban = optional_single_text(elements, 2);
    if account_number.is_none() && iban.is_none() {
        return Err(Error::MissingValue {
            field: "UPD account identity",
        });
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

    Ok(Account {
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
    })
}

fn parse_balance_tan_requirement(segment: &Segment) -> Result<Option<bool>, Error> {
    let Some(parameters) = segment.elements().get(4) else {
        return Err(Error::MissingValue {
            field: "HIPINS parameters",
        });
    };
    let components = parameters.components();
    for pair in components.get(5..).unwrap_or_default().chunks_exact(2) {
        if optional_component(pair, 0).as_deref() == Some("HKSAL") {
            return match optional_component(pair, 1).as_deref() {
                Some("J") => Ok(Some(true)),
                Some("N") => Ok(Some(false)),
                _ => Err(Error::InvalidValue {
                    field: "HKSAL TAN requirement",
                }),
            };
        }
    }
    Ok(None)
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
    if method_values.is_empty() || !method_values.len().is_multiple_of(width) {
        return Err(Error::InvalidResponse {
            structure: "HITANS method parameters",
        });
    }
    method_values
        .chunks_exact(width)
        .map(|components| parse_tan_method(components, version))
        .collect()
}

fn parse_tan_method(components: &[Value], version: u16) -> Result<TanMethod, Error> {
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
