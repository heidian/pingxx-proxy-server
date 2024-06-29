use crate::core::PaymentChannel;
use std::str::FromStr;

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

pub async fn create_or_update_sub_app_channel(
    prisma_client: &crate::prisma::PrismaClient,
    app_id: String,
    sub_app_id: String,
    channel: String,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    {
        let params = params.clone();
        match PaymentChannel::from_str(&channel).map_err(|e| format!("invalid channel: {:?}", e))? {
            PaymentChannel::AlipayPcDirect => {
                serde_json::from_value::<crate::alipay::AlipayPcDirectConfig>(params)
                    .map_err(|e| format!("invalid alipay_pc_direct params: {:?}", e))?;
            }
            PaymentChannel::AlipayWap => {
                serde_json::from_value::<crate::alipay::AlipayWapConfig>(params)
                    .map_err(|e| format!("invalid alipay_wap params: {:?}", e))?;
            }
            PaymentChannel::WxPub => {
                serde_json::from_value::<crate::weixin::WxPubConfig>(params)
                    .map_err(|e| format!("invalid wx_pub params: {:?}", e))?;
            }
            PaymentChannel::WxLite => {
                serde_json::from_value::<crate::weixin::WxLiteConfig>(params)
                    .map_err(|e| format!("invalid wx_lite params: {:?}", e))?;
            }
        };
    }

    prisma_client
        .channel_params()
        .upsert(
            crate::prisma::channel_params::app_id_sub_app_id_channel(
                app_id.clone(),
                sub_app_id.clone(),
                channel.clone(),
            ),
            crate::prisma::channel_params::create(
                channel.clone(),
                params.clone(),
                vec![
                    crate::prisma::channel_params::app_id::set(Some(app_id.to_string())),
                    crate::prisma::channel_params::sub_app_id::set(Some(sub_app_id.to_string())),
                ],
            ),
            vec![crate::prisma::channel_params::params::set(params)],
        )
        .exec()
        .await
        .map_err(|e| format!("sql error: {:?}", e))?;

    Ok(serde_json::json!({}))
}
