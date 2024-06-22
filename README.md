# 平替部分 Ping++ 接口的支付网关服务

Ping++ 价格谈崩了，打算做个平替的接口，目的是可以不动业务代码并复用 sdk 直接替换，到时候就换个支付回调地址和 ping++ sdk 的 api base，顺便 RIIR ✌️

完全替代不大可能，Ping++ 依然是目前接触到的接入最全的支付网关，主要实现：

第一优先级

-   支付宝 openapi 和 mapi 两种接口格式和 rsa/rsa256 两种签名方式
-   微信公众号和小程序
-   退款

第二优先级

-   支付宝微信代扣
-   当面付，App 支付（比较难测试）
-   境外支付宝微信
-   PayPal
-   查账接口

第三阶段

-   Dashboard
-   分叉，不再兼容

## 调试方式

1. 启动 frp, 将本地服务暴露到 pingxx.heidianapi.com
2. 启动 shopbackend 和 shopbackend django admin
3. 启动 shopfront 以前端发起支付
4. 启动 pingxx-proxy-server

## 启动 pingxx-proxy-server

日志用了 tracing 库，需要设置环境变量 RUST_LOG，比如

```bash
RUST_LOG=pingxx_proxy_server=debug cargo watch -x "run"
```

## 数据结构

**Credential**

前端需要的参数，用于客户端打开支付控件、支付页面、显示二维码等。

```js
{
    object: "credential",
    alipay_pc_direct: {
        // alipay_pc_direct 所需的参数
    },
    wx_pub: {
        // wx_pub 所需的参数
    },
}
```

**Charge**

```js
{
    id,
    object: "charge",
    channel,  // 支付渠道
    credential,  // Credential 对象
}
```

**Order**

```js
{
    id,
    object: "order",
    charge_essentials: {
        // 最近一次请求的支付所需的支付要素，是 Charge 上的部分数据，但不是完整的 Charge 对象
        channel,
        credential,  // Credential 对象
    },
    charges: {
        data: [
            // Charge 列表, 和前面 Charge 结构完全一样
        ]
    },
}
```
