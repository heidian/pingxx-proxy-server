use crate::core::{
    ChannelChargeExtra, ChannelChargeRequest, ChannelHandler, ChargeError, PaymentChannel,
};
use crate::{alipay, weixin};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, Debug)]
pub struct CreateChargeRequestPayload {
    pub app: String,      // ping++ 的商户系统的 appid
    pub order_no: String, // 商户订单号
    pub channel: PaymentChannel,
    pub amount: i32,
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
    Ok(json!({}))
}
