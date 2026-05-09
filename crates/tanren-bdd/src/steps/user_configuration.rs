//! User-configuration/credential step definitions for B-0048.
//!
//! Like the account-flow steps, every operation dispatches through the
//! per-interface [`AccountHarness`](tanren_testkit::AccountHarness)
//! trait so each interface tag exercises its harness implementation.

use cucumber::{then, when};
use secrecy::SecretString;
use tanren_configuration_secrets::{
    OwnerScope, ThemePreference, USER_CREDENTIAL_SECRET_MAX_BYTES, USER_SETTING_EDITOR_MAX_BYTES,
    UserCredentialKind, UserSettingKey, UserSettingValue,
};
use tanren_contract::{CreateUserCredentialRequest, UpsertUserSettingRequest};
use tanren_identity_policy::AccountId;
use tanren_testkit::{HarnessOutcome, record_failure};

use crate::{AccountContext, TanrenWorld};

#[when(expr = "{word} lists user settings for their own account")]
async fn when_list_settings_own(world: &mut TanrenWorld, actor: String) {
    let requested = {
        let ctx = world.ensure_account_ctx().await;
        actor_account_id(ctx, &actor)
    };
    list_user_settings(world, actor, requested).await;
}

#[when(expr = "{word} lists user settings for {word}'s account")]
async fn when_list_settings_for_actor(world: &mut TanrenWorld, actor: String, target: String) {
    let requested = {
        let ctx = world.ensure_account_ctx().await;
        actor_account_id(ctx, &target)
    };
    list_user_settings(world, actor, requested).await;
}

#[when(expr = "{word} sets the {word} setting to {string}")]
async fn when_set_setting(world: &mut TanrenWorld, actor: String, key: String, value: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = actor_account_id(ctx, &actor);
    let request = UpsertUserSettingRequest {
        key: parse_setting_key(&key).expect("setting key must be supported by scenario"),
        value: parse_setting_value(&key, &value).expect("setting value must be supported"),
    };
    let result = ctx.harness.upsert_user_setting(account_id, request).await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(response) => {
            upsert_local_setting(entry, response.setting.key, response.setting.value.clone());
            HarnessOutcome::Other("setting_updated".to_owned())
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} sets the editor setting to an oversized value")]
async fn when_set_editor_setting_oversized(world: &mut TanrenWorld, actor: String) {
    let oversized = "e".repeat(USER_SETTING_EDITOR_MAX_BYTES + 1);
    when_set_setting(world, actor, "editor".to_owned(), oversized).await;
}

#[when(expr = "{word} lists user credentials for their own account")]
async fn when_list_credentials_own(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = actor_account_id(ctx, &actor);
    let result = ctx.harness.list_user_credentials(account_id).await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(response) => {
            entry.last_user_credentials = response.items;
            HarnessOutcome::Other("credentials_listed".to_owned())
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} adds a {word} user credential with value {string}")]
async fn when_add_credential(world: &mut TanrenWorld, actor: String, kind: String, value: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = actor_account_id(ctx, &actor);
    let request = CreateUserCredentialRequest {
        kind: parse_credential_kind(&kind).expect("credential kind must be supported by scenario"),
        owner_scope: OwnerScope::User { account_id },
        value: SecretString::from(value),
    };
    let result = ctx.harness.add_user_credential(account_id, request).await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(response) => {
            entry.remembered_credential_id = Some(response.item.id.clone());
            entry
                .last_user_credentials
                .retain(|item| item.id != response.item.id);
            entry.last_user_credentials.push(response.item);
            HarnessOutcome::Other("credential_added".to_owned())
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[when(expr = "{word} adds a {word} user credential with an oversized value")]
async fn when_add_credential_oversized(world: &mut TanrenWorld, actor: String, kind: String) {
    let oversized = "s".repeat(USER_CREDENTIAL_SECRET_MAX_BYTES + 1);
    when_add_credential(world, actor, kind, oversized).await;
}

#[when(expr = "{word} removes their remembered user credential")]
async fn when_remove_credential(world: &mut TanrenWorld, actor: String) {
    let ctx = world.ensure_account_ctx().await;
    let account_id = actor_account_id(ctx, &actor);
    let item_id = ctx
        .actors
        .get(&actor)
        .and_then(|state| state.remembered_credential_id.as_deref())
        .expect("actor must have a remembered credential id")
        .to_owned();
    let result = ctx
        .harness
        .remove_user_credential(account_id, &item_id)
        .await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(response) => {
            entry
                .last_user_credentials
                .retain(|item| item.id != response.item.id);
            HarnessOutcome::Other("credential_removed".to_owned())
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

#[then(expr = "{word} sees {int} user settings")]
async fn then_sees_setting_count(world: &mut TanrenWorld, actor: String, count: usize) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have settings snapshot");
    assert_eq!(
        entry.last_user_settings.len(),
        count,
        "expected {actor} to have {count} user settings, got {actual}",
        actual = entry.last_user_settings.len()
    );
}

#[then(expr = "{word} sees the {word} setting set to {string}")]
async fn then_sees_setting(world: &mut TanrenWorld, actor: String, key: String, value: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have settings snapshot");
    let expected_key = parse_setting_key(&key).expect("setting key must be supported by scenario");
    let expected_value =
        parse_setting_value(&key, &value).expect("setting value must be supported");
    assert!(
        entry
            .last_user_settings
            .iter()
            .any(|setting| setting.key == expected_key && setting.value == expected_value),
        "expected {actor} to include {key}={value}; got settings={:?} outcome={:?}",
        entry.last_user_settings,
        ctx.last_outcome
    );
}

#[then(expr = "{word} does not see the {word} setting set to {string}")]
async fn then_does_not_see_setting(
    world: &mut TanrenWorld,
    actor: String,
    key: String,
    value: String,
) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have settings snapshot");
    let expected_key = parse_setting_key(&key).expect("setting key must be supported by scenario");
    let expected_value =
        parse_setting_value(&key, &value).expect("setting value must be supported");
    assert!(
        entry
            .last_user_settings
            .iter()
            .all(|setting| !(setting.key == expected_key && setting.value == expected_value)),
        "expected {actor} to exclude {key}={value}; got {:?}",
        entry.last_user_settings
    );
}

#[then(expr = "{word} sees {int} user credential metadata rows")]
async fn then_sees_credential_count(world: &mut TanrenWorld, actor: String, count: usize) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have credentials snapshot");
    assert_eq!(
        entry.last_user_credentials.len(),
        count,
        "expected {actor} to have {count} credential metadata rows, got {actual}; outcome={outcome:?}",
        actual = entry.last_user_credentials.len(),
        outcome = ctx.last_outcome
    );
}

#[then(expr = "{word} sees a {word} user credential metadata row")]
async fn then_sees_credential_kind(world: &mut TanrenWorld, actor: String, kind: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have credentials snapshot");
    let expected =
        parse_credential_kind(&kind).expect("credential kind must be supported by scenario");
    assert!(
        entry
            .last_user_credentials
            .iter()
            .any(|item| item.kind == expected),
        "expected {actor} to include a {kind} metadata row; got {:?}",
        entry.last_user_credentials
    );
}

#[then(expr = "{word} does not see credential value {string}")]
async fn then_not_see_credential_value(world: &mut TanrenWorld, actor: String, value: String) {
    let ctx = world.ensure_account_ctx().await;
    let entry = ctx
        .actors
        .get(&actor)
        .expect("actor must have credentials snapshot");
    let snapshot = serde_json::to_string(&entry.last_user_credentials)
        .expect("credential metadata snapshot should serialize");
    assert!(
        !snapshot.contains(&value),
        "credential secret should not appear in metadata snapshot"
    );
}

async fn list_user_settings(world: &mut TanrenWorld, actor: String, requested: AccountId) {
    let ctx = world.ensure_account_ctx().await;
    let result = ctx.harness.list_user_settings(requested).await;
    let entry = ctx.actors.entry(actor).or_default();
    let outcome = match result {
        Ok(response) => {
            entry.last_user_settings = response.items;
            HarnessOutcome::Other("settings_listed".to_owned())
        }
        Err(err) => record_failure(err, entry),
    };
    ctx.last_outcome = Some(outcome);
}

fn actor_account_id(ctx: &AccountContext, actor: &str) -> AccountId {
    let entry = ctx
        .actors
        .get(actor)
        .expect("actor must have prior account state");
    entry
        .sign_in
        .as_ref()
        .map(|session| session.account_id)
        .or_else(|| entry.sign_up.as_ref().map(|session| session.account_id))
        .or_else(|| {
            entry
                .accept_invitation
                .as_ref()
                .map(|acceptance| acceptance.session.account_id)
        })
        .expect("actor must have a known account id")
}

fn parse_setting_key(raw: &str) -> Option<UserSettingKey> {
    match raw {
        "theme" => Some(UserSettingKey::Theme),
        "editor" => Some(UserSettingKey::Editor),
        _ => None,
    }
}

fn parse_setting_value(key: &str, raw: &str) -> Option<UserSettingValue> {
    match parse_setting_key(key)? {
        UserSettingKey::Theme => Some(UserSettingValue::Theme(match raw {
            "system" => ThemePreference::System,
            "light" => ThemePreference::Light,
            "dark" => ThemePreference::Dark,
            _ => return None,
        })),
        UserSettingKey::Editor => Some(UserSettingValue::Editor(raw.to_owned())),
    }
}

fn parse_credential_kind(raw: &str) -> Option<UserCredentialKind> {
    match raw {
        "provider_api_token" => Some(UserCredentialKind::ProviderApiToken),
        "harness_api_token" => Some(UserCredentialKind::HarnessApiToken),
        _ => None,
    }
}

fn upsert_local_setting(
    entry: &mut tanren_testkit::ActorState,
    key: UserSettingKey,
    value: UserSettingValue,
) {
    if let Some(existing) = entry
        .last_user_settings
        .iter_mut()
        .find(|setting| setting.key == key)
    {
        existing.value = value;
        return;
    }
    entry
        .last_user_settings
        .push(tanren_contract::UserSettingView {
            key,
            value,
            updated_at: chrono::Utc::now(),
        });
}
