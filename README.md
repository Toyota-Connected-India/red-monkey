# red-monkey
A Redis Fault Injection Proxy

![Red monkey](https://github.com/toyotaconnected-India/red-monkey/workflows/red-monkey/badge.svg?branch=main)


<p align="center">
  <img src="./assets/red-monkey-logo.png" width=300 height=300 />
</p>

## Why red-monkey? 

[[Todo]]

## How to build and run locally? 

### Build 

```
make build 
```

### Run 

```
make run
```

### Environment varialbles

1. `proxy_port` is the port at which the redis requests are proxied to the origin redis server. 
2. `redis_address` is the address of the origin redis server.
3. `is_tls_on` is the boolean value which says whether to establish TLS connection to the origin redis server from `red-monkey`.
4. The HTTP fault configuration server address and port are configured in the `Rocket.toml` file.


## Usage

### Fault configuration

The fault configuration payload and endpoints can be found in the Swagger file [here](docs/swagger-fault-config-server.yaml).

[[Todo]]

