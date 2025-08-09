use log::{debug, error};
use proxy_wasm::hostcalls;
use std::collections::HashMap;

// Simple metric IDs
static mut AUTHORIZED_CALLS_METRIC_ID: Option<u32> = None;
static mut LIMITED_CALLS_METRIC_ID: Option<u32> = None;
static mut TOKEN_USAGE_METRIC_ID: Option<u32> = None;

// Storage for user/group specific metrics
static mut USER_GROUP_METRICS: Option<HashMap<String, u32>> = None;

pub fn initialize_metrics() {
    debug!("Initializing custom metrics");

    // Initialize user/group metrics storage
    unsafe {
        USER_GROUP_METRICS = Some(HashMap::new());
    }

    // Authorized calls counter
    match hostcalls::define_metric(
        proxy_wasm::types::MetricType::Counter,
        "authorized_calls_total",
    ) {
        Ok(metric_id) => {
            unsafe {
                AUTHORIZED_CALLS_METRIC_ID = Some(metric_id);
            }
            debug!(
                "Defined authorized_calls_total metric with ID: {}",
                metric_id
            );
        }
        Err(e) => {
            error!("Failed to define authorized_calls_total metric: {:?}", e);
        }
    }

    // Limited calls counter
    match hostcalls::define_metric(
        proxy_wasm::types::MetricType::Counter,
        "limited_calls_total",
    ) {
        Ok(metric_id) => {
            unsafe {
                LIMITED_CALLS_METRIC_ID = Some(metric_id);
            }
            debug!("Defined limited_calls_total metric with ID: {}", metric_id);
        }
        Err(e) => {
            error!("Failed to define limited_calls_total metric: {:?}", e);
        }
    }

    // Token usage counter
    match hostcalls::define_metric(
        proxy_wasm::types::MetricType::Counter,
        "token_usage_total",
    ) {
        Ok(metric_id) => {
            unsafe {
                TOKEN_USAGE_METRIC_ID = Some(metric_id);
            }
            debug!("Defined token_usage_total metric with ID: {}", metric_id);
        }
        Err(e) => {
            error!("Failed to define token_usage_total metric: {:?}", e);
        }
    }
}

pub fn increment_authorized_calls() {
    unsafe {
        if let Some(metric_id) = AUTHORIZED_CALLS_METRIC_ID {
            match hostcalls::increment_metric(metric_id, 1) {
                Ok(_) => debug!("Incremented authorized_calls_total metric"),
                Err(e) => error!("Failed to increment authorized_calls_total metric: {:?}", e),
            }
        } else {
            error!("Authorized calls metric not initialized");
        }
    }
}

pub fn increment_limited_calls() {
    unsafe {
        if let Some(metric_id) = LIMITED_CALLS_METRIC_ID {
            match hostcalls::increment_metric(metric_id, 1) {
                Ok(_) => debug!("Incremented limited_calls_total metric"),
                Err(e) => error!("Failed to increment limited_calls_total metric: {:?}", e),
            }
        } else {
            error!("Limited calls metric not initialized");
        }
    }
}

pub fn increment_token_usage(tokens: i64) {
    debug!("Incrementing token usage by {} tokens", tokens);
    unsafe {
        if let Some(metric_id) = TOKEN_USAGE_METRIC_ID {
            match hostcalls::increment_metric(metric_id, tokens) {
                Ok(_) => debug!("Incremented token_usage_total metric by {} tokens", tokens),
                Err(e) => error!("Failed to increment token_usage_total metric: {:?}", e),
            }
        } else {
            error!("Token usage metric not initialized");
        }
    }
}

// Helper function to log all response headers for debugging
fn debug_log_all_response_headers() {
    debug!("=== ALL RESPONSE HEADERS ===");
    match proxy_wasm::hostcalls::get_map(proxy_wasm::types::MapType::HttpResponseHeaders) {
        Ok(headers) => {
            for (key, value) in headers {
                debug!("Response Header: {} = {}", key, value);
            }
            debug!("=== END RESPONSE HEADERS ===");
        }
        Err(e) => {
            debug!("Failed to get response headers: {:?}", e);
        }
    }
}

// Helper function to extract token count from response headers
fn extract_token_count_from_headers() -> Option<i64> {
    debug!("Attempting to extract token count from response headers");

    // Log all headers for debugging
    debug_log_all_response_headers();

    // Common token usage headers to check
    let token_headers = [
        "x-usage-prompt-tokens",
        "x-usage-completion-tokens", 
        "x-usage-total-tokens",
        "x-ratelimit-tokens",
        "x-tokens-used",
        "usage-tokens",
        "total-tokens"
    ];
    
    // Get all response headers as a map
    match proxy_wasm::hostcalls::get_map(proxy_wasm::types::MapType::HttpResponseHeaders) {
        Ok(headers) => {
            for header_name in &token_headers {
                // Search through the headers Vec<(String, String)> for our target header
                for (key, value) in &headers {
                    if key.to_lowercase() == header_name.to_lowercase() {
                        debug!("Found token header {}: {}", header_name, value);
                        match value.parse::<i64>() {
                            Ok(tokens) => {
                                debug!("Successfully parsed {} tokens from header {}", tokens, header_name);
                                return Some(tokens);
                            }
                            Err(e) => {
                                debug!("Failed to parse token count from header {} value '{}': {:?}", header_name, value, e);
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            debug!("Failed to get response headers: {:?}", e);
        }
    }
    
    debug!("No token count found in response headers");
    None
}

// Helper function to extract user info from auth metadata
fn extract_user_info() -> (String, String) {
    debug!("Attempting to extract user info from auth metadata");

    // Use the correct path format for wasm kuadrant attributes
    let user_id_path = crate::data::wasm_prop(&["auth", "identity", "userid"]);
    let user_id_result = crate::data::get_attribute::<String>(&user_id_path);
    let user_id = match user_id_result {
        Ok(Some(id)) => {
            debug!("Found user_id: {}", id);
            id
        }
        Ok(None) => {
            debug!("user_id attribute exists but is None");
            "unknown".to_string()
        }
        Err(e) => {
            debug!("Error getting user_id: {:?}", e);
            "unknown".to_string()
        }
    };

    let user_groups_path = crate::data::wasm_prop(&["auth", "identity", "groups"]);
    let user_groups_result = crate::data::get_attribute::<String>(&user_groups_path);
    let user_groups = match user_groups_result {
        Ok(Some(groups)) => {
            debug!("Found user_groups: {}", groups);
            groups
        }
        Ok(None) => {
            debug!("user_groups attribute exists but is None");
            "unknown".to_string()
        }
        Err(e) => {
            debug!("Error getting user_groups: {:?}", e);
            "unknown".to_string()
        }
    };

    // Extract first group if multiple groups are comma-separated
    let group = user_groups
        .split(',')
        .next()
        .unwrap_or("unknown")
        .to_string();

    debug!("Extracted user_id: {}, group: {}", user_id, group);
    (user_id, group)
}

// Helper function to get or create a user/group specific metric with cleaner names
fn get_or_create_user_group_metric(
    metric_type: &str,
    user: &str,
    group: &str,
    namespace: &str,
) -> Option<u32> {
    // Format: metric_type__user__USER__group__GROUP__namespace__NAMESPACE
    let metric_name = format!(
        "{}__user__{}__group__{}__namespace__{}",
        metric_type, user, group, namespace
    );

    let map_key = format!("{}:{}:{}:{}", metric_type, user, group, namespace);

    unsafe {
        if let Some(ref mut metrics_map) = USER_GROUP_METRICS {
            if let Some(&metric_id) = metrics_map.get(&map_key) {
                return Some(metric_id);
            }

            // Create new metric
            match hostcalls::define_metric(proxy_wasm::types::MetricType::Counter, &metric_name) {
                Ok(metric_id) => {
                    debug!("Defined user/group metric: {} with ID: {} for user: {}, group: {}, namespace: {}", 
                           metric_name, metric_id, user, group, namespace);
                    metrics_map.insert(map_key, metric_id);
                    Some(metric_id)
                }
                Err(e) => {
                    error!(
                        "Failed to define user/group metric {}: {:?}",
                        metric_name, e
                    );
                    None
                }
            }
        } else {
            error!("User/group metrics not initialized");
            None
        }
    }
}

pub fn increment_authorized_calls_with_user_group(scope: &str) {
    debug!(
        "increment_authorized_calls_with_user_group called for scope: {}",
        scope
    );
    let (user_id, group) = extract_user_info();

    if user_id == "unknown" || group == "unknown" {
        debug!("Skipping user/group metric due to unknown user or group");
        return;
    }

    // For auth calls, use a fixed namespace since scope contains a hash
    // From the logs we can see the actual namespace is "llm-d"
    let namespace = "llm-d".to_string();

    if let Some(metric_id) = get_or_create_user_group_metric(
        "authorized_calls_with_user_and_group",
        &user_id,
        &group,
        &namespace,
    ) {
        match hostcalls::increment_metric(metric_id, 1) {
            Ok(_) => debug!("Incremented authorized_calls_with_user_and_group for user: {}, group: {}, namespace: {}", user_id, group, namespace),
            Err(e) => error!("Failed to increment authorized_calls_with_user_and_group: {:?}", e),
        }
    }
}

pub fn increment_limited_calls_with_user_group(scope: &str) {
    debug!(
        "increment_limited_calls_with_user_group called for scope: {}",
        scope
    );
    let (user_id, group) = extract_user_info();

    if user_id == "unknown" || group == "unknown" {
        debug!("Skipping user/group metric due to unknown user or group");
        return;
    }

    // For rate limit actions, the scope contains the correct namespace
    // From logs: "llm-d/ms-sim-llm-d-modelservice"
    // Let's clean it up to just use the first part: "llm-d"
    let namespace = scope.split('/').next().unwrap_or(scope).to_string();

    if let Some(metric_id) = get_or_create_user_group_metric(
        "limited_calls_with_user_and_group",
        &user_id,
        &group,
        &namespace,
    ) {
        match hostcalls::increment_metric(metric_id, 1) {
            Ok(_) => debug!("Incremented limited_calls_with_user_and_group for user: {}, group: {}, namespace: {}", user_id, group, namespace),
            Err(e) => error!("Failed to increment limited_calls_with_user_and_group: {:?}", e),
        }
    }
}

pub fn increment_token_usage_with_user_group(tokens: i64, scope: &str) {
    debug!(
        "increment_token_usage_with_user_group called with {} tokens for scope: {}",
        tokens, scope
    );
    let (user_id, group) = extract_user_info();

    if user_id == "unknown" || group == "unknown" {
        debug!("Skipping user/group token metric due to unknown user or group");
        return;
    }

    // For token usage, we'll use "llm-d" as the namespace since this is typically from the LLM response
    let namespace = "llm-d".to_string();

    if let Some(metric_id) = get_or_create_user_group_metric(
        "token_usage_with_user_and_group",
        &user_id,
        &group,
        &namespace,
    ) {
        match hostcalls::increment_metric(metric_id, tokens) {
            Ok(_) => debug!("Incremented token_usage_with_user_and_group by {} tokens for user: {}, group: {}, namespace: {}", tokens, user_id, group, namespace),
            Err(e) => error!("Failed to increment token_usage_with_user_and_group: {:?}", e),
        }
    }
}

pub fn process_response_headers_for_token_usage() {
    debug!("Processing response headers for token usage tracking");
    
    if let Some(token_count) = extract_token_count_from_headers() {
        debug!("Found {} tokens in response headers, recording metrics", token_count);
        
        // Increment basic token usage counter
        increment_token_usage(token_count);
        
        // Increment user/group specific token usage counter
        // Using empty scope since we don't have a specific scope context here
        increment_token_usage_with_user_group(token_count, "");
    } else {
        debug!("No token usage information found in response headers");
    }
}
