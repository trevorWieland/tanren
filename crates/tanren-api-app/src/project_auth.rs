pub(crate) fn legacy_scope_mismatch(
    legacy_owning_account_id: Option<tanren_identity_policy::AccountId>,
    actor_account_id: tanren_identity_policy::AccountId,
) -> bool {
    legacy_owning_account_id.is_some_and(|owning| owning != actor_account_id)
}
