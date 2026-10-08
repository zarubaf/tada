//! Facts (ADR 0049): the field catalog of an event, its event profile and the typed accessors of the reserved core fields.

use std::fmt::Debug;

use async_trait::async_trait;
use jiff::Timestamp;
use serde_json::{Value as Json, json};
use tada_domain::RecordVersion;
use tada_domain::facts::{
    Currency, DateWindow, Decimal, FactState, FactValue, FieldDefinition, Granularity,
    ReferenceTarget, ShortText, Unit, ValueType, Valued, core_catalog,
};
use tada_domain::ids::{
    ChangesetId, EventId, FactId, FactVersionId, FieldDefinitionId, OpenQuestionId, ProposalId,
    UserId,
};
use tada_domain::proposals::QuestionText;
use tada_domain::sources::Evidence;

use crate::access::{self, AccessError, Principal};
use crate::caller::OrgScope;
use crate::identity::IdentityStore;
use crate::store::StoreError;

/// The current fact versions of one event, with their fields and evidence,
/// and apart from them the open fact proposals and the open questions of the event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventProfile {
    /// One entry for each fact of the event, in the order of the field keys.
    pub fields: Vec<ProfileEntry>,
    /// The fact proposals of the event without a review result, oldest first.
    /// They are not accepted state (ADR 0050). A proposal can name a field that has no fact yet.
    pub proposals: Vec<OpenProposalRef>,
    /// The open questions of the event, in the order of their event-local numbers.
    pub open_questions: Vec<OpenQuestionRef>,
}

/// An open proposal that sets the fact of a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenProposalRef {
    pub proposal_id: ProposalId,
    pub changeset_id: ChangesetId,
    pub field_id: FieldDefinitionId,
    pub state: FactState<Valued>,
    /// The fact version that the proposal expects. `None` means that the field has no fact yet.
    pub expected_version: Option<RecordVersion>,
    pub created_at: Timestamp,
}

/// An open question of an event, with its event-local number `QST-<n>` (ADR 0038).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenQuestionRef {
    pub id: OpenQuestionId,
    pub local_number: u64,
    pub text: QuestionText,
    pub owner: UserId,
    pub version: RecordVersion,
}

impl OpenQuestionRef {
    /// The event-local ID, for example `QST-001` (ADR 0038).
    pub fn readable_id(&self) -> String {
        format!("QST-{:03}", self.local_number)
    }
}

/// The current version of one fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileEntry {
    pub field: FieldDefinition,
    pub fact_id: FactId,
    /// The number of the current fact version.
    pub version: RecordVersion,
    pub state: FactState<Valued>,
    /// The evidence links of the fact version.
    pub evidence: Vec<Evidence>,
}

/// The current version of one fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactVersionRef {
    pub id: FactVersionId,
    pub fact_id: FactId,
    pub number: RecordVersion,
    pub state: FactState<Valued>,
}

/// The repository port for field definitions and facts. Each method stays inside `scope`.
/// Shipped field definitions have no organization; each organization reads them (ADR 0049).
#[async_trait]
pub trait FactStore: Debug + Send + Sync {
    /// The field catalog of the event: the shipped fields and the fields of the event, in the order of their keys.
    async fn catalog(
        &self,
        scope: OrgScope,
        event: EventId,
    ) -> Result<Vec<FieldDefinition>, StoreError>;

    /// The current fact versions of the event with their evidence, its open fact proposals and its open questions.
    async fn profile(&self, scope: OrgScope, event: EventId) -> Result<EventProfile, StoreError>;

    /// The current version of the fact of `field` in the event, or `None` if the event has no such fact.
    async fn current_version(
        &self,
        scope: OrgScope,
        event: EventId,
        field: FieldDefinitionId,
    ) -> Result<Option<FactVersionRef>, StoreError>;

    /// The fact versions of `versions` that exist in the event, each as a fact and its version number.
    async fn existing_versions(
        &self,
        scope: OrgScope,
        event: EventId,
        versions: &[(FactId, RecordVersion)],
    ) -> Result<Vec<(FactId, RecordVersion)>, StoreError>;
}

/// The event profile: the current facts of the event, for each caller who can read the event.
pub async fn get_event_profile(
    caller: &impl Principal,
    event: EventId,
    identity: &dyn IdentityStore,
    store: &dyn FactStore,
) -> Result<EventProfile, AccessError> {
    let access = access::event_access(caller, event, identity).await?;
    if !access.can_read() {
        return Err(AccessError::NotFound);
    }
    Ok(store.profile(caller.scope(), event).await?)
}

/// The field catalog of the event, for each caller who can read the event.
pub async fn get_field_catalog(
    caller: &impl Principal,
    event: EventId,
    identity: &dyn IdentityStore,
    store: &dyn FactStore,
) -> Result<Vec<FieldDefinition>, AccessError> {
    let access = access::event_access(caller, event, identity).await?;
    if !access.can_read() {
        return Err(AccessError::NotFound);
    }
    Ok(store.catalog(caller.scope(), event).await?)
}

/// The JSON Schema of a value of a field with `value_type`: the `ValueInput` of a proposal, narrowed to this field.
/// The API and the MCP tools give it to clients, so that an agent can propose a valid value (ADR 0040).
pub fn value_schema(value_type: &ValueType) -> Json {
    let date = || json!({"type": "string", "format": "date"});
    let decimal = |unit: &Unit| {
        json!({
            "type": "string",
            "pattern": format!(r"^-?[0-9]+(\.[0-9]{{1,{}}})?$", Decimal::MAX_SCALE),
            "description": format!("A decimal number in the unit `{}`.", unit.as_str()),
        })
    };
    let money = |currency: &Currency| {
        json!({
            "type": "integer",
            "description": format!("An amount in the minor unit of `{}`.", currency.as_str()),
        })
    };
    let (tag, properties) = match value_type {
        ValueType::Text => (
            "text",
            json!({"text": {"type": "string", "minLength": 1, "maxLength": ShortText::MAX_CHARS}}),
        ),
        ValueType::Boolean => ("boolean", json!({"value": {"type": "boolean"}})),
        ValueType::Quantity { unit } => (
            "quantity",
            json!({"min": decimal(unit), "max": decimal(unit)}),
        ),
        ValueType::Money { currency } => (
            "money",
            json!({"min": money(currency), "max": money(currency)}),
        ),
        ValueType::Date => ("date", json!({"date": date()})),
        ValueType::DateWindow { granularity } => {
            let granularity = match granularity {
                Some(granularity) => json!({"const": granularity_name(*granularity)}),
                None => json!({"enum": ["day", "week", "month"]}),
            };
            (
                "date-window",
                json!({"start": date(), "end": date(), "granularity": granularity}),
            )
        }
        ValueType::Choice { values, multiple } => {
            let keys: Vec<&str> = values.iter().map(|value| value.key.as_str()).collect();
            let mut list = json!({
                "type": "array",
                "items": {"enum": keys},
                "minItems": 1,
                "uniqueItems": true,
            });
            if !multiple {
                list["maxItems"] = json!(1);
            }
            ("choice", json!({"keys": list}))
        }
        ValueType::Reference { target } => {
            let target = match target {
                ReferenceTarget::Document => "document",
                ReferenceTarget::Event => "event",
            };
            (
                "reference",
                json!({"target": {"const": target}, "id": {"type": "string", "format": "uuid"}}),
            )
        }
    };
    let mut properties = properties;
    properties["type"] = json!({"const": tag});
    let mut required: Vec<&String> = properties
        .as_object()
        .map(|properties| properties.keys().collect())
        .unwrap_or_default();
    required.sort();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

/// The API name of a granularity (ADR 0044).
fn granularity_name(granularity: Granularity) -> &'static str {
    match granularity {
        Granularity::Day => "day",
        Granularity::Week => "week",
        Granularity::Month => "month",
    }
}

/// The value type of a reserved core field does not match its typed accessor.
#[derive(Debug, thiserror::Error)]
#[error("the fact of the reserved field {0} holds a value of another value type")]
struct ReservedFieldMismatch(&'static str);

/// The state of the reserved core field `date_window` of the event (ADR 0049).
/// An event without this fact gives `Unknown`.
/// The caller must have access to the event, because this accessor does not check it.
pub async fn date_window(
    scope: OrgScope,
    event: EventId,
    store: &dyn FactStore,
) -> Result<FactState<DateWindow>, StoreError> {
    const KEY: &str = "date_window";
    let Some(current) = store.current_version(scope, event, core_field(KEY)).await? else {
        return Ok(FactState::Unknown);
    };
    let window = |valued: Valued| match valued.value {
        FactValue::DateWindow(window) => Ok(window),
        _ => Err(StoreError::Internal(Box::new(ReservedFieldMismatch(KEY)))),
    };
    Ok(match current.state {
        FactState::Accepted(valued) => FactState::Accepted(window(valued)?),
        FactState::Assumption(valued) => FactState::Assumption(window(valued)?),
        FactState::Unknown => FactState::Unknown,
    })
}

/// The ID of a field of the shipped core catalog.
fn core_field(key: &str) -> FieldDefinitionId {
    core_catalog()
        .into_iter()
        .find(|field| field.key.as_str() == key)
        .map(|field| field.id)
        .expect("each reserved key is a core field")
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use jiff::civil::date;
    use tada_domain::facts::Granularity;
    use tada_domain::identity::{EventRole, OrganizationRole};
    use tada_domain::ids::{OrganizationId, UserId};
    use uuid::Uuid;

    use tada_domain::facts::ReferenceTarget;

    use super::*;
    use crate::caller::MemberCaller;
    use crate::identity::{Membership, UserRef};
    use crate::proposals::ValueInput;

    /// One event of one organization. The store answers with `current` and an empty profile.
    #[derive(Debug, Default)]
    struct Memory {
        current: Mutex<Option<FactVersionRef>>,
        asked_field: Mutex<Option<FieldDefinitionId>>,
    }

    fn testwil() -> OrganizationId {
        OrganizationId::from_uuid(Uuid::from_u128(10))
    }

    fn open_day() -> EventId {
        EventId::from_uuid(Uuid::from_u128(20))
    }

    #[async_trait]
    impl FactStore for Memory {
        async fn catalog(
            &self,
            _: OrgScope,
            _: EventId,
        ) -> Result<Vec<FieldDefinition>, StoreError> {
            Ok(core_catalog())
        }

        async fn profile(&self, _: OrgScope, _: EventId) -> Result<EventProfile, StoreError> {
            Ok(EventProfile {
                fields: Vec::new(),
                proposals: Vec::new(),
                open_questions: Vec::new(),
            })
        }

        async fn current_version(
            &self,
            _: OrgScope,
            _: EventId,
            field: FieldDefinitionId,
        ) -> Result<Option<FactVersionRef>, StoreError> {
            *self.asked_field.lock().unwrap() = Some(field);
            Ok(self.current.lock().unwrap().clone())
        }

        async fn existing_versions(
            &self,
            _: OrgScope,
            _: EventId,
            _: &[(FactId, RecordVersion)],
        ) -> Result<Vec<(FactId, RecordVersion)>, StoreError> {
            unreachable!()
        }
    }

    #[async_trait]
    impl IdentityStore for Memory {
        async fn user(&self, _: UserId) -> Result<Option<UserRef>, StoreError> {
            unreachable!()
        }

        async fn memberships_of(&self, _: UserId) -> Result<Vec<Membership>, StoreError> {
            unreachable!()
        }

        async fn membership(
            &self,
            _: OrgScope,
            _: UserId,
        ) -> Result<Option<OrganizationRole>, StoreError> {
            unreachable!()
        }

        async fn event_exists(&self, scope: OrgScope, event: EventId) -> Result<bool, StoreError> {
            Ok(scope.organization_id() == testwil() && event == open_day())
        }

        async fn event_role(
            &self,
            _: OrgScope,
            _: EventId,
            _: UserId,
        ) -> Result<Option<EventRole>, StoreError> {
            Ok(None)
        }

        async fn event_roles_of(
            &self,
            _: OrgScope,
            _: UserId,
        ) -> Result<Vec<(EventId, EventRole)>, StoreError> {
            unreachable!()
        }
    }

    fn caller(role: OrganizationRole) -> MemberCaller {
        MemberCaller::new(UserId::from_uuid(Uuid::from_u128(1)), testwil(), role)
    }

    fn current(state: FactState<Valued>) -> FactVersionRef {
        FactVersionRef {
            id: FactVersionId::from_uuid(Uuid::from_u128(30)),
            fact_id: FactId::from_uuid(Uuid::from_u128(31)),
            number: RecordVersion::FIRST,
            state,
        }
    }

    fn may_to_june() -> DateWindow {
        DateWindow::new(date(2030, 5, 1), date(2030, 6, 30), Granularity::Month).unwrap()
    }

    #[tokio::test]
    async fn a_reader_of_the_event_gets_its_profile() {
        let memory = Memory::default();
        let owner = caller(OrganizationRole::Owner);
        let profile = get_event_profile(&owner, open_day(), &memory, &memory).await;
        assert_eq!(
            profile.unwrap(),
            EventProfile {
                fields: Vec::new(),
                proposals: Vec::new(),
                open_questions: Vec::new(),
            }
        );
    }

    #[tokio::test]
    async fn a_member_without_an_event_role_does_not_find_the_profile() {
        let memory = Memory::default();
        let member = caller(OrganizationRole::Member);
        let profile = get_event_profile(&member, open_day(), &memory, &memory).await;
        assert!(matches!(profile, Err(AccessError::NotFound)));
    }

    #[tokio::test]
    async fn a_reader_of_the_event_gets_its_field_catalog() {
        let memory = Memory::default();
        let owner = caller(OrganizationRole::Owner);
        let catalog = get_field_catalog(&owner, open_day(), &memory, &memory).await;
        assert_eq!(catalog.unwrap(), core_catalog());
        let member = caller(OrganizationRole::Member);
        let catalog = get_field_catalog(&member, open_day(), &memory, &memory).await;
        assert!(matches!(catalog, Err(AccessError::NotFound)));
    }

    /// The variants of `ValueInput`: the value of `type` and the names of the properties of each.
    fn value_input_variants() -> Vec<(String, Vec<String>, Vec<String>)> {
        let schema = serde_json::to_value(schemars::schema_for!(ValueInput)).unwrap();
        schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|variant| {
                let tag = variant["properties"]["type"]["const"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                (tag, keys(variant), required(variant))
            })
            .collect()
    }

    fn keys(schema: &serde_json::Value) -> Vec<String> {
        let mut keys: Vec<String> = schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    }

    fn required(schema: &serde_json::Value) -> Vec<String> {
        let mut required: Vec<String> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|key| key.as_str().unwrap().to_owned())
            .collect();
        required.sort();
        required
    }

    fn each_value_type() -> Vec<ValueType> {
        let mut types: Vec<ValueType> = core_catalog()
            .into_iter()
            .map(|field| field.value_type)
            .collect();
        types.extend([
            ValueType::Boolean,
            ValueType::Date,
            ValueType::Reference {
                target: ReferenceTarget::Document,
            },
        ]);
        types
    }

    #[test]
    fn the_schema_of_each_value_type_has_the_shape_of_its_value_input() {
        let variants = value_input_variants();
        let mut tags = Vec::new();
        for value_type in each_value_type() {
            let schema = value_schema(&value_type);
            let tag = schema["properties"]["type"]["const"].as_str().unwrap();
            let (_, input_keys, input_required) = variants
                .iter()
                .find(|(variant, _, _)| variant == tag)
                .unwrap_or_else(|| panic!("no value input of the type {tag}"));
            assert_eq!(&keys(&schema), input_keys, "{tag}");
            assert_eq!(&required(&schema), input_required, "{tag}");
            assert_eq!(schema["additionalProperties"], false, "{tag}");
            tags.push(tag.to_owned());
        }
        tags.sort();
        tags.dedup();
        assert_eq!(
            tags.len(),
            variants.len(),
            "a value input without a value type"
        );
    }

    #[test]
    fn the_schema_of_a_choice_lists_its_keys_and_limits_a_single_choice() {
        let audience = core_catalog()
            .into_iter()
            .find(|field| field.key.as_str() == "audience")
            .unwrap();
        let schema = value_schema(&audience.value_type);
        let keys = &schema["properties"]["keys"];
        assert_eq!(
            keys["items"]["enum"],
            serde_json::json!(["public", "members", "invited"])
        );
        assert_eq!(keys["minItems"], 1);
        assert_eq!(keys["maxItems"], 1);
        let window = value_schema(&ValueType::DateWindow {
            granularity: Some(Granularity::Day),
        });
        assert_eq!(
            window["properties"]["granularity"],
            serde_json::json!({"const": "day"})
        );
    }

    #[test]
    fn an_open_question_has_a_readable_id_with_three_digits() {
        let question = |local_number| OpenQuestionRef {
            id: OpenQuestionId::from_uuid(Uuid::from_u128(40)),
            local_number,
            text: QuestionText::parse("Welcher Samstag?").unwrap(),
            owner: UserId::from_uuid(Uuid::from_u128(1)),
            version: RecordVersion::FIRST,
        };
        assert_eq!(question(1).readable_id(), "QST-001");
        assert_eq!(question(1234).readable_id(), "QST-1234");
    }

    #[tokio::test]
    async fn the_date_window_reads_the_core_field_and_is_unknown_without_a_fact() {
        let memory = Memory::default();
        let scope = caller(OrganizationRole::Owner).scope();
        let state = date_window(scope, open_day(), &memory).await.unwrap();
        assert_eq!(state, FactState::Unknown);
        assert_eq!(
            *memory.asked_field.lock().unwrap(),
            Some(core_field("date_window"))
        );
    }

    #[tokio::test]
    async fn the_date_window_keeps_the_state_of_the_fact() {
        let memory = Memory::default();
        let scope = caller(OrganizationRole::Owner).scope();
        let valued = Valued {
            value: FactValue::DateWindow(may_to_june()),
            approximate: false,
        };
        *memory.current.lock().unwrap() = Some(current(FactState::Assumption(valued)));
        let state = date_window(scope, open_day(), &memory).await.unwrap();
        assert_eq!(state, FactState::Assumption(may_to_june()));
    }

    #[tokio::test]
    async fn a_date_window_fact_with_another_value_type_is_an_internal_error() {
        let memory = Memory::default();
        let scope = caller(OrganizationRole::Owner).scope();
        let valued = Valued {
            value: FactValue::Boolean(true),
            approximate: false,
        };
        *memory.current.lock().unwrap() = Some(current(FactState::Accepted(valued)));
        let state = date_window(scope, open_day(), &memory).await;
        assert!(matches!(state, Err(StoreError::Internal(_))));
    }
}
