use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct ListResponse<T: Serialize> {
    object: String,
    url: String,
    has_more: bool,
    data: Vec<T>,
}

pub mod order {
    use super::charge::ChargeResponse;
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
        pub charges: ListResponse<ChargeResponse>,
    }

    type T = (OrderData, Vec<ChargeData>, AppData, SubAppData);
    impl From<T> for OrderResponse {
        fn from((order, charges, app, sub_app): T) -> Self {
            let charges = {
                // let empty: Vec<crate::prisma::charge::Data> = vec![];
                // let charges = order.charges.as_ref().unwrap_or(&empty);
                let data = charges
                    .iter()
                    // .filter_map(|charge| {
                    //     let charge_response: ChargeResponse = charge.to_owned().into();
                    //     match serde_json::to_value(charge_response) {
                    //         Ok(res) => Some(res),
                    //         Err(_) => None,
                    //     }
                    // })
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
                charges,
            }
        }
    }
}

pub mod charge {
    use super::*;
    use crate::prisma::charge::Data as ChargeData;

    #[derive(Serialize, Debug)]
    pub struct ChargeResponse {
        pub id: String,
        pub object: String,
        pub channel: String,
        pub amount: i32,
        pub extra: serde_json::Value,
        pub credential: serde_json::Value,
    }

    type T = ChargeData;
    impl From<T> for ChargeResponse {
        fn from(charge: T) -> Self {
            Self {
                id: charge.id,
                object: "charge".to_string(),
                channel: charge.channel,
                amount: charge.amount,
                extra: charge.extra,
                credential: charge.credential,
            }
        }
    }
}
