use umi_api::method_name::MethodName;

/// Grants access to all endpoints except engine API
pub(super) fn http(method: &MethodName) -> bool {
    method.is_non_engine_api()
}

/// Grants access to all endpoints
pub(super) fn auth(_method: &MethodName) -> bool {
    true
}
