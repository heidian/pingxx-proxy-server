use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub enum PaymentChannel {
    #[serde(rename = "alipay_pc_direct")]
    AlipayPcDirect,
    #[serde(rename = "alipay_wap")]
    AlipayWap,
    #[serde(rename = "wx_pub")]
    WxPub,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct ChargeExtra {
    pub success_url: Option<String>,
    pub cancel_url: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CreateChargeRequestPayload {
    pub charge_amount: u32,
    pub channel: PaymentChannel,
    pub extra: ChargeExtra,
}
