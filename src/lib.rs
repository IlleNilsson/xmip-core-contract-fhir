#![forbid(unsafe_code)]

//! The FHIR content contract — a technology of `xmip-core-contract`.
//!
//! Two claims, decided 2026-09-07 (ADR-0042): **well-formedness is a given**
//! and **conformance is a given once a contract is named**.
//!
//! Well-formed here is a *FHIR JSON resource*: a JSON object with a
//! `resourceType` string, an `id` that is a FHIR id when present, and, for a
//! `Bundle`, an `entry` array whose every entry carries a `resource` that is
//! itself well-formed. Conformance is the resource type: a Location that names
//! this contract with `Patient` bound has every resource held to that type, and
//! `Patient:R4` holds `meta.profile` or the bound release's structure
//! definition base. Every FHIR release is a version of this one technology and
//! lives here; the XML form arrives when the `xml-schema` contract is bound
//! with the FHIR schemas, not as a second technology.
//!
//! Profiles, the `StructureDefinition`s a national base or an implementation
//! guide adds, are the next layer here, bound the way a schema is.

use contract::{
    Contract, ContractDescriptor, ContractError, ContractFactory, ContractId, ValidationIssue,
    ValidationResult,
};
use serde_json::Value;
use stream::Stream;

/// The bound resource type, with the release when named: `Patient`,
/// `Patient:R4`, `Bundle:R5`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceType {
    pub name: String,
    pub release: Option<String>,
}

impl ResourceType {
    /// `Patient` or `Patient:R4`.
    ///
    /// # Errors
    /// An empty type, or a release that is not `R4`, `R4B` or `R5`.
    pub fn parse(reference: &str) -> Result<Self, ContractError> {
        let (name, release) = reference
            .split_once(':')
            .map_or((reference, None), |(n, r)| {
                (n, Some(r.trim().to_ascii_uppercase()))
            });
        let name = name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(ContractError {
                message: format!("{reference:?} is not a resource type"),
            });
        }
        if let Some(release) = &release
            && !matches!(release.as_str(), "R4" | "R4B" | "R5")
        {
            return Err(ContractError {
                message: format!("{release} is not a FHIR release this contract knows"),
            });
        }
        Ok(Self {
            name: name.to_string(),
            release,
        })
    }

    fn reference(&self) -> String {
        match &self.release {
            Some(release) => format!("{}:{release}", self.name),
            None => self.name.clone(),
        }
    }
}

/// The FHIR contract, bare or bound to a resource type.
pub struct Fhir {
    descriptor: ContractDescriptor,
    resource_type: Option<ResourceType>,
}

impl Fhir {
    /// Any well-formed resource.
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: descriptor("fhir"),
            resource_type: None,
        }
    }

    /// A well-formed resource of `resource_type`.
    #[must_use]
    pub fn of(resource_type: ResourceType) -> Self {
        Self {
            descriptor: descriptor(&format!("fhir:{}", resource_type.reference())),
            resource_type: Some(resource_type),
        }
    }
}

impl Default for Fhir {
    fn default() -> Self {
        Self::new()
    }
}

fn descriptor(id: &str) -> ContractDescriptor {
    ContractDescriptor {
        id: ContractId(id.to_string()),
        version: "1".to_string(),
        representation: "application/fhir+json".to_string(),
    }
}

fn issue(code: &str, message: &str, path: &str) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        message: message.to_string(),
        path: Some(path.to_string()),
    }
}

/// A FHIR id: up to 64 characters of letters, digits, `-` and `.`.
fn is_id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 64
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

/// Well-formedness of one resource, recursing into a Bundle's entries.
fn check_resource(value: &Value, path: &str, out: &mut Vec<ValidationIssue>) {
    let Value::Object(members) = value else {
        out.push(issue("malformed", "a resource is a JSON object", path));
        return;
    };
    let Some(kind) = members.get("resourceType").and_then(Value::as_str) else {
        out.push(issue(
            "malformed",
            "no resourceType",
            &format!("{path}/resourceType"),
        ));
        return;
    };
    if let Some(id) = members.get("id")
        && !id.as_str().is_some_and(is_id)
    {
        out.push(issue(
            "malformed",
            "id is not a FHIR id",
            &format!("{path}/id"),
        ));
    }
    if kind == "Bundle" {
        match members.get("entry") {
            None => {}
            Some(Value::Array(entries)) => {
                for (index, entry) in entries.iter().enumerate() {
                    let at = format!("{path}/entry/{index}/resource");
                    match entry.get("resource") {
                        Some(resource) => check_resource(resource, &at, out),
                        None => out.push(issue("malformed", "an entry without a resource", &at)),
                    }
                }
            }
            Some(_) => out.push(issue(
                "malformed",
                "entry is not an array",
                &format!("{path}/entry"),
            )),
        }
    }
}

/// The release a resource declares through `meta.profile`, if it names the
/// HL7 base structure definition: `.../R4/...` says nothing, but a base URL
/// `http://hl7.org/fhir/StructureDefinition/Patient` carries no release, so
/// only `fhirVersion` in a `CapabilityStatement` or a profile with `|4.0.1`
/// style version can say. This reads the latter.
fn declared_release(value: &Value) -> Option<String> {
    let profiles = value.pointer("/meta/profile")?.as_array()?;
    profiles
        .iter()
        .filter_map(Value::as_str)
        .find_map(|profile| {
            let (_, version) = profile.rsplit_once('|')?;
            Some(match version.split('.').next()? {
                "4" => if version.starts_with("4.3") {
                    "R4B"
                } else {
                    "R4"
                }
                .to_string(),
                "5" => "R5".to_string(),
                other => other.to_string(),
            })
        })
}

impl Contract for Fhir {
    fn descriptor(&self) -> &ContractDescriptor {
        &self.descriptor
    }

    fn identify(&self, stream: &Stream) -> Result<bool, ContractError> {
        if stream.media_type().is_some_and(|m| {
            m.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .eq_ignore_ascii_case("application/fhir+json")
        }) {
            return Ok(true);
        }
        Ok(std::str::from_utf8(stream.bytes()).is_ok_and(|t| t.contains("\"resourceType\"")))
    }

    fn validate(&self, stream: &Stream) -> Result<ValidationResult, ContractError> {
        let mut issues = Vec::new();
        let value: Value = match serde_json::from_slice(stream.bytes()) {
            Ok(value) => value,
            Err(error) => {
                return Ok(result(vec![issue(
                    "malformed",
                    &format!("not valid JSON: {error}"),
                    &format!("line {} column {}", error.line(), error.column()),
                )]));
            }
        };
        check_resource(&value, "", &mut issues);
        if let (Some(wanted), true) = (&self.resource_type, issues.is_empty()) {
            let actual = value
                .get("resourceType")
                .and_then(Value::as_str)
                .unwrap_or("");
            if actual != wanted.name {
                issues.push(issue(
                    "resource-type",
                    &format!("is {actual}, the contract is {}", wanted.name),
                    "/resourceType",
                ));
            }
            if let (Some(release), Some(declared)) = (&wanted.release, declared_release(&value))
                && declared != *release
            {
                issues.push(issue(
                    "release",
                    &format!("declares {declared} through meta.profile, the contract is {release}"),
                    "/meta/profile",
                ));
            }
        }
        Ok(result(issues))
    }
}

fn result(issues: Vec<ValidationIssue>) -> ValidationResult {
    ValidationResult {
        valid: issues.is_empty(),
        issues,
    }
}

/// Loads the contract a Location names: an empty reference is the bare
/// contract, anything else a resource type, `Patient` or `Patient:R4`.
pub struct FhirFactory;

impl ContractFactory for FhirFactory {
    fn technology(&self) -> &'static str {
        "fhir"
    }

    fn load(&self, reference: &str) -> Result<Box<dyn Contract>, ContractError> {
        if reference.trim().is_empty() {
            return Ok(Box::new(Fhir::new()));
        }
        Ok(Box::new(Fhir::of(ResourceType::parse(reference)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xcore::StreamId;

    fn stream(text: &str) -> Stream {
        Stream::new(
            StreamId::new(1),
            text.as_bytes().to_vec(),
            Some("application/fhir+json".into()),
        )
    }

    const PATIENT: &str = concat!(
        r#"{"resourceType":"Patient","id":"p-1","name":[{"family":"Doe"}],"#,
        r#""meta":{"profile":["http://hl7.org/fhir/StructureDefinition/Patient|4.0.1"]}}"#
    );

    #[test]
    fn a_resource_holds_bare_and_a_bundle_is_checked_through() {
        let fhir = Fhir::new();
        assert!(fhir.validate(&stream(PATIENT)).expect("validates").valid);
        let bundle = format!(
            concat!(
                r#"{{"resourceType":"Bundle","type":"collection","entry":[{{"resource":{}}},"#,
                r#"{{"resource":{{"name":"no type"}}}}]}}"#
            ),
            PATIENT
        );
        let held = fhir.validate(&stream(&bundle)).expect("validates");
        assert_eq!(
            held.issues[0].path.as_deref(),
            Some("/entry/1/resource/resourceType")
        );
        let bad_id = r#"{"resourceType":"Patient","id":"has space"}"#;
        assert_eq!(
            fhir.validate(&stream(bad_id)).expect("validates").issues[0]
                .path
                .as_deref(),
            Some("/id")
        );
    }

    #[test]
    fn a_bound_type_and_release_hold_and_name_the_departure() {
        let patient = Fhir::of(ResourceType::parse("Patient:R4").expect("type"));
        assert_eq!(patient.descriptor().id.0, "fhir:Patient:R4");
        assert!(patient.validate(&stream(PATIENT)).expect("validates").valid);
        let observation = Fhir::of(ResourceType::parse("Observation").expect("type"));
        assert_eq!(
            observation
                .validate(&stream(PATIENT))
                .expect("validates")
                .issues[0]
                .code,
            "resource-type"
        );
        let r5 = Fhir::of(ResourceType::parse("Patient:r5").expect("type"));
        assert_eq!(
            r5.validate(&stream(PATIENT)).expect("validates").issues[0].code,
            "release"
        );
        assert!(ResourceType::parse("Patient:R2").is_err());
        assert!(ResourceType::parse("").is_err());
    }

    #[test]
    fn identifies_by_media_type_or_resource_type_key() {
        let fhir = Fhir::new();
        assert!(fhir.identify(&stream(PATIENT)).expect("identifies"));
        let plain = Stream::new(StreamId::new(1), PATIENT.as_bytes().to_vec(), None);
        assert!(fhir.identify(&plain).expect("identifies"));
        let other = Stream::new(StreamId::new(1), b"{\"a\":1}".to_vec(), None);
        assert!(!fhir.identify(&other).expect("identifies"));
    }

    #[test]
    fn the_factory_reads_the_type_off_the_reference() {
        let factory = FhirFactory;
        assert_eq!(factory.technology(), "fhir");
        assert_eq!(factory.load("").expect("bare").descriptor().id.0, "fhir");
        assert_eq!(
            factory.load("Bundle:R5").expect("typed").descriptor().id.0,
            "fhir:Bundle:R5"
        );
        assert!(factory.load("Not a type").is_err());
    }
}
