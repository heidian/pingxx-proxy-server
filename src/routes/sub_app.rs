// use serde_json::json;

pub async fn retrieve_sub_app(
    prisma_client: &crate::prisma::PrismaClient,
    app_id: String,
    sub_app_id: String,
) -> Result<serde_json::Value, String> {
    let sub_app = prisma_client
        .sub_app()
        .find_unique(crate::prisma::sub_app::id::equals(sub_app_id.to_string()))
        .with(crate::prisma::sub_app::app::fetch())
        .with(crate::prisma::sub_app::channel_params::fetch(vec![]))
        .exec()
        .await
        .map_err(|e| format!("sql error: {:?}", e))?
        .ok_or_else(|| format!("sub_app {} not found", sub_app_id))?;
    let app = sub_app
        .app
        .clone()
        .ok_or_else(|| "sub_app has no parent app".to_string())?;
    if app_id != app.id {
        return Err("sub_app doesn't belong to app".to_string());
    }
    let json_data: serde_json::Value = sub_app
        .try_into()
        .map_err(|e| format!("error serializing sub_app: {:?}", e))?;
    Ok(json_data)
}
