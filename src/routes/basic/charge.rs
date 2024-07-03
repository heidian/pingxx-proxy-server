use crate::core::{
    ChannelChargeExtra, ChannelChargeRequest, ChannelHandler, ChargeError, ChargeResponse,
    PaymentChannel,
};
use crate::{alipay, weixin};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Debug)]
pub struct App {
    id: String,
}

#[derive(Deserialize, Debug)]
pub struct CreateChargeRequestPayload {
    pub app: App, // ping++ 的商户系统的 appid
    #[serde(rename = "order_no")]
    pub merchant_order_no: String, // 商户订单号
    pub channel: PaymentChannel,
    #[serde(rename = "amount")]
    pub charge_amount: i32,
    pub client_ip: String,
    pub currency: String,
    pub subject: String,
    pub body: String,
    pub time_expire: i32,
    pub extra: ChannelChargeExtra,
}

pub async fn create_charge(
    prisma_client: &crate::prisma::PrismaClient,
    charge_req_payload: CreateChargeRequestPayload,
) -> Result<serde_json::Value, ChargeError> {
    let charge_id = crate::utils::generate_id("ch_");

    let app = prisma_client
        .app()
        .find_unique(crate::prisma::app::id::equals(charge_req_payload.app.id))
        .exec()
        .await
        .map_err(|e| ChargeError::InternalError(format!("sql error: {:?}", e)))?
        .ok_or_else(|| ChargeError::MalformedRequest("app not found".to_string()))?;

    let handler: Box<dyn ChannelHandler + Send> = match charge_req_payload.channel {
        PaymentChannel::AlipayPcDirect => {
            Box::new(alipay::AlipayPcDirect::new(&prisma_client, Some(&app.id), None).await?)
        }
        PaymentChannel::AlipayWap => {
            Box::new(alipay::AlipayWap::new(&prisma_client, Some(&app.id), None).await?)
        }
        PaymentChannel::WxPub => {
            Box::new(weixin::WxPub::new(&prisma_client, Some(&app.id), None).await?)
        }
        PaymentChannel::WxLite => {
            Box::new(weixin::WxLite::new(&prisma_client, Some(&app.id), None).await?)
        }
    };

    let credential_object = handler
        .create_credential(&ChannelChargeRequest {
            charge_id: &charge_id,
            charge_amount: charge_req_payload.charge_amount,
            merchant_order_no: &charge_req_payload.merchant_order_no,
            client_ip: &charge_req_payload.client_ip,
            time_expire: charge_req_payload.time_expire,
            subject: &charge_req_payload.subject,
            body: &charge_req_payload.body,
            extra: &charge_req_payload.extra,
        })
        .await?;

    let credential = {
        let mut credential = json!({
            "object": "credential",
            // [channel]: credential_object
        });
        let key = serde_json::to_value(&charge_req_payload.channel)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        credential[key] = credential_object;
        credential
    };

    let extra = serde_json::to_value(charge_req_payload.extra).map_err(|e| {
        ChargeError::MalformedRequest(format!("error serializing charge extra: {:?}", e))
    })?;

    let charge = prisma_client
        .charge()
        .create(
            charge_id.clone(),
            crate::prisma::app::id::equals(app.id.clone()),
            charge_req_payload.channel.to_string(),
            charge_req_payload.merchant_order_no,
            false,
            charge_req_payload.charge_amount,
            charge_req_payload.client_ip,
            charge_req_payload.subject,
            charge_req_payload.body,
            charge_req_payload.currency,
            extra,
            credential,
            charge_req_payload.time_expire,
            vec![],
        )
        .exec()
        .await
        .map_err(|e| ChargeError::InternalError(format!("sql error: {:?}", e)))?;

    let charge_response: ChargeResponse = (&charge, &app).into();
    let mut result = serde_json::to_value(charge_response).map_err(|e| {
        ChargeError::InternalError(format!("error serializing order response payload: {:?}", e))
    })?;
    result["order_no"] = result["merchant_order_no"].clone();

    Ok(result)
}
