use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct ListResponse<T: Serialize> {
    object: String,
    url: String,
    has_more: bool,
    data: Vec<T>,
}

pub mod order {
    use super::charge::{ChargeResponse, ChargeEssentialsResponse};
    use super::*;
    use crate::prisma::{
        app::Data as AppData, charge::Data as ChargeData, order::Data as OrderData,
        sub_app::Data as SubAppData,
    };

    #[derive(Serialize, Debug)]
    pub struct OrderResponse {
        pub id: String,
        pub object: String,
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

    type T = (OrderData, Option<ChargeData>, Vec<ChargeData>, AppData, SubAppData);
    impl From<T> for OrderResponse {
        fn from((order, charge, charges, app, sub_app): T) -> Self {
            let charges = {
                // let empty: Vec<crate::prisma::charge::Data> = vec![];
                // let charges = order.charges.as_ref().unwrap_or(&empty);
                let data = charges
                    .iter()
                    .map(|charge| {
                        let charge_response: ChargeResponse = charge.to_owned().into();
                        charge_response
                    })
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
            Self {
                id: order.id,
                object: String::from("order"),
                created: order.created_at.timestamp() as i32,
                app: app.id,
                receipt_app: sub_app.id.clone(),
                service_app: sub_app.id,
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
                time_paid: None,
                time_expire: order.time_expire,
                metadata: order.metadata,
                charge_essentials,
                charges,
            }
        }
    }
}

pub mod charge {
    use super::*;
    use crate::prisma::charge::Data as ChargeData;

    #[derive(Serialize, Debug)]
    pub struct ChargeEssentialsResponse {
        pub channel: String,
        pub extra: serde_json::Value,
        pub credential: serde_json::Value,
        pub failure_code: Option<String>,
        pub failure_msg: Option<String>,
    }

    impl From<ChargeData> for ChargeEssentialsResponse {
        fn from(charge: ChargeData) -> Self {
            Self {
                channel: charge.channel,
                extra: charge.extra,
                credential: charge.credential,
                failure_code: charge.failure_code,
                failure_msg: charge.failure_msg,
            }
        }
    }

    #[derive(Serialize, Debug)]
    pub struct ChargeResponse {
        pub id: String,
        pub object: String,
        pub channel: String,
        pub merchant_order_no: String,
        pub paid: bool,
        pub amount: i32,
        pub client_ip: String,
        pub subject: String,
        pub body: String,
        pub currency: String,
        pub extra: serde_json::Value,
        pub credential: serde_json::Value,
        pub failure_code: Option<String>,
        pub failure_msg: Option<String>,
    }

    impl From<ChargeData> for ChargeResponse {
        fn from(charge: ChargeData) -> Self {
            Self {
                id: charge.id,
                object: "charge".to_string(),
                channel: charge.channel,
                merchant_order_no: charge.merchant_order_no,
                paid: charge.paid,
                amount: charge.amount,
                client_ip: charge.client_ip,
                subject: charge.subject,
                body: charge.body,
                currency: charge.currency,
                extra: charge.extra,
                credential: charge.credential,
                failure_code: charge.failure_code,
                failure_msg: charge.failure_msg,
            }
        }
    }
}
