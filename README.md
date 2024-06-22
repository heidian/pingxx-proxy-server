# 平替部分 Ping++ 接口的支付网关服务

Ping++ 价格谈崩了，打算做个平替的接口，目的是可以不动业务代码并复用 sdk 直接替换，到时候就换个支付回调地址和 ping++ sdk 的 api base，顺便 RIIR ✌️

完全替代不大可能，Ping++ 依然是目前接触到的接入最全的支付网关，主要实现：

第一优先级
- 支付宝 openapi 和 mapi 两种接口格式和 rsa/rsa256 两种签名方式
- 微信公众号和小程序
- 退款

第二优先级
- 支付宝微信代扣
- 当面付，App支付（比较难测试）
- 境外支付宝微信
- PayPal
- 查账接口

第三阶段
- Dashboard
- 分叉，不再兼容

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
