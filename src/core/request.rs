use super::error::{ChargeError, RefundError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::str::FromStr;

#[async_trait]
pub trait ChannelHandler {
    async fn create_credential(
        &self,
        request: &ChannelChargeRequest,
    ) -> Result<serde_json::Value, ChargeError>;

    fn process_charge_notify(&self, payload: &str) -> Result<ChargeStatus, ChargeError>;

    async fn create_refund(
        &self,
        request: &ChannelRefundRequest,
    ) -> Result<RefundResult, RefundError>;

    fn process_refund_notify(&self, payload: &str) -> Result<RefundStatus, RefundError>;
}

pub struct ChannelChargeRequest<'a> {
    pub charge_id: &'a str,
    pub charge_amount: i32,
    pub merchant_order_no: &'a str,
    pub client_ip: &'a str,
    pub time_expire: i32, // 过期时间 timestamp 精确到秒
    pub subject: &'a str,
    pub body: &'a str,
    pub extra: &'a ChannelChargeExtra,
}

/**
 * 请求支付时渠道相关的额外参数
 */
#[derive(Deserialize, Serialize, Debug)]
pub struct ChannelChargeExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_id: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum ChargeStatus {
    Success,
    Fail,
}

pub struct ChannelRefundRequest<'a> {
    pub charge_id: &'a str,
    pub charge_amount: i32,
    pub refund_id: &'a str,
    pub refund_amount: i32,
    pub merchant_order_no: &'a str,
    pub description: &'a str,
    pub extra: &'a ChannelRefundExtra,
}

/**
 * 请求退款时渠道相关的额外参数
 */
#[derive(Deserialize, Serialize, Debug)]
pub struct ChannelRefundExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub funding_source: Option<String>, // 微信退款专用 unsettled_funds | recharge_funds
}

#[derive(Deserialize, Serialize, Debug, PartialEq)]
pub enum RefundStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "succeeded")]
    Success,
    #[serde(rename = "failed")]
    Fail,
}

impl FromStr for RefundStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val = serde_json::Value::String(s.to_string());
        let channel = serde_json::from_value::<RefundStatus>(val)
            .map_err(|e| format!("error parsing RefundStatus from string: {:?}", e))?;
        Ok(channel)
    }
}

impl ToString for RefundStatus {
    fn to_string(&self) -> String {
        let val = serde_json::to_value(self).unwrap();
        val.as_str().unwrap().to_string()
    }
}

#[derive(Debug)]
pub struct RefundResult {
    pub status: RefundStatus,
    pub amount: i32,
    pub description: String,
    pub extra: serde_json::Value,
    pub failure_code: Option<String>,
    pub failure_msg: Option<String>,
}

impl Default for RefundResult {
    fn default() -> Self {
        RefundResult {
            status: RefundStatus::Pending,
            amount: 0,
            description: "".to_string(),
            extra: json!({}),
            failure_code: None,
            failure_msg: None,
        }
    }
}
