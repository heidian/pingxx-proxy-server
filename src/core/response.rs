use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct ListResponse<T: Serialize> {
    object: String,
    url: String,
    has_more: bool,
    data: Vec<T>,
}

pub mod order {
    use super::charge::{ChargeEssentialsResponse, ChargeResponse};
    use super::*;
    use crate::prisma::{
        app::Data as AppData, charge::Data as ChargeData, order::Data as OrderData,
        sub_app::Data as SubAppData,
    };

    #[derive(Serialize, Debug)]
    pub struct OrderResponse {
        pub id: String,
        pub object: String,
        pub api_base: String,
        pub created: i32,
        pub app: String,
        pub receipt_app: String,
        pub service_app: String,
        pub uid: String,
        pub merchant_order_no: String,
        pub status: String,
        pub paid: bool,
        pub refunded: bool,
        pub amount: i32,
        pub amount_paid: i32,
        pub amount_refunded: i32,
        pub client_ip: String,
        pub subject: String,
        pub body: String,
        pub currency: String,
        pub time_paid: Option<i32>,
        pub time_expire: i32,
        pub metadata: serde_json::Value,
        pub charge_essentials: Option<ChargeEssentialsResponse>,
        pub charges: ListResponse<ChargeResponse>,
    }

    type T<'a> = (
        &'a OrderData,
        Option<&'a ChargeData>,
        &'a Vec<ChargeData>,
        &'a AppData,
        &'a SubAppData,
    );
    impl From<T<'_>> for OrderResponse {
        fn from((order, charge, charges, app, sub_app): T) -> Self {
            // order response 里不需要 refunds
            let refunds: Vec<crate::prisma::refund::Data> = vec![];
            let charges = {
                // let empty: Vec<crate::prisma::charge::Data> = vec![];
                // let charges = order.charges.as_ref().unwrap_or(&empty);
                let data = charges
                    .iter()
                    .map(|charge| (charge, &refunds, app).into())
                    .collect::<Vec<ChargeResponse>>();
                ListResponse {
                    object: "list".to_string(),
                    url: "/v1/charges".to_string(),
                    has_more: false,
                    data,
                }
            };
            let charge_essentials = match charge {
                Some(charge) => Some(charge.into()),
                None => None,
            };
            let order = order.clone();
            Self {
                id: order.id,
                object: String::from("order"),
                api_base: crate::utils::api_base(),
                created: order.created_at.timestamp() as i32,
                app: app.id.clone(),
                receipt_app: sub_app.id.clone(),
                service_app: sub_app.id.clone(),
                uid: order.uid,
                merchant_order_no: order.merchant_order_no,
                status: order.status,
                paid: order.paid,
                refunded: order.refunded,
                amount: order.amount,
                amount_paid: order.amount_paid,
                amount_refunded: order.amount_refunded,
                client_ip: order.client_ip,
                subject: order.subject,
                body: order.body,
                currency: order.currency,
                time_paid: order.time_paid,
                time_expire: order.time_expire,
                metadata: order.metadata,
                charge_essentials,
                charges,
            }
        }
    }
}

pub mod charge {
    use super::refund::RefundResponse;
    use super::*;
    use crate::prisma::{
        app::Data as AppData, charge::Data as ChargeData, refund::Data as RefundData,
    };

    #[derive(Serialize, Debug)]
    pub struct ChargeEssentialsResponse {
        pub channel: String,
        pub extra: serde_json::Value,
        pub credential: serde_json::Value,
        pub failure_code: Option<String>,
        pub failure_msg: Option<String>,
    }

    impl From<&ChargeData> for ChargeEssentialsResponse {
        fn from(charge: &ChargeData) -> Self {
            Self {
                channel: charge.channel.clone(),
                extra: charge.extra.clone(),
                credential: charge.credential.clone(),
                failure_code: charge.failure_code.clone(),
                failure_msg: charge.failure_msg.clone(),
            }
        }
    }

    #[derive(Serialize, Debug)]
    pub struct ChargeResponse {
        pub id: String,
        pub object: String,
        pub api_base: String,
        pub app: String,
        pub channel: String,
        pub order_no: String,  // 兼容 basic 和 order 的 charge 接口, basic 接口上的商户订单号是 order_no
        pub merchant_order_no: String,
        pub paid: bool,
        pub amount: i32,
        pub client_ip: String,
        pub subject: String,
        pub body: String,
        pub currency: String,
        pub extra: serde_json::Value,
        pub credential: serde_json::Value,
        pub time_paid: Option<i32>,
        pub time_expire: i32,
        pub failure_code: Option<String>,
        pub failure_msg: Option<String>,
        pub refunds: ListResponse<RefundResponse>,
    }

    type T<'a> = (&'a ChargeData, &'a Vec<RefundData>, &'a AppData);
    impl From<T<'_>> for ChargeResponse {
        fn from((charge, refunds, app): T) -> Self {
            let charge = charge.clone();
            let refunds = {
                let data = refunds
                    .iter()
                    .map(|refund| (refund, &charge).into())
                    .collect::<Vec<RefundResponse>>();
                ListResponse {
                    object: "list".to_string(),
                    url: format!("/v1/charges/{}/refunds", &charge.id),
                    has_more: false,
                    data,
                }
            };
            Self {
                id: charge.id,
                object: "charge".to_string(),
                api_base: crate::utils::api_base(),
                channel: charge.channel,
                app: app.id.clone(),
                order_no: charge.merchant_order_no.clone(),
                merchant_order_no: charge.merchant_order_no,
                paid: charge.paid,
                amount: charge.amount,
                client_ip: charge.client_ip,
                subject: charge.subject,
                body: charge.body,
                currency: charge.currency,
                extra: charge.extra,
                credential: charge.credential,
                time_paid: charge.time_paid,
                time_expire: charge.time_expire,
                failure_code: charge.failure_code,
                failure_msg: charge.failure_msg,
                refunds,
            }
        }
    }
}

pub mod refund {
    use super::*;
    use crate::prisma::{charge::Data as ChargeData, refund::Data as RefundData};

    #[derive(Serialize, Debug)]
    pub struct RefundResponse {
        pub id: String,
        pub object: String,
        pub api_base: String,
        pub amount: i32,
        pub succeed: bool,
        pub status: String,
        pub description: String,
        pub charge: String,          // charge.id
        pub charge_order_no: String, // charge.merchant_order_no
        pub extra: serde_json::Value,
        pub time_succeed: Option<i32>,
        pub failure_code: Option<String>,
        pub failure_msg: Option<String>,
    }

    type T<'a> = (&'a RefundData, &'a ChargeData);
    impl From<T<'_>> for RefundResponse {
        fn from((refund, charge): T) -> Self {
            let refund = refund.clone();
            Self {
                id: refund.id,
                object: "refund".to_string(),
                api_base: crate::utils::api_base(),
                amount: charge.amount,
                succeed: refund.status == "succeeded",
                status: refund.status,
                description: refund.description,
                charge: charge.id.clone(),
                charge_order_no: charge.merchant_order_no.clone(),
                extra: refund.extra,
                time_succeed: refund.time_succeed,
                failure_code: refund.failure_code,
                failure_msg: refund.failure_msg,
            }
        }
    }
}
