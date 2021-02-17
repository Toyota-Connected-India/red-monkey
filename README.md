# red-monkey
A Redis Fault Injection Proxy

![Red monkey](https://github.com/toyotaconnected-India/red-monkey/workflows/red-monkey/badge.svg?branch=main)


<p align="center">
  <img src="./assets/red-monkey-logo.png" width=300 height=300 />
</p>


## Usage:

```bash
$ docker-compose up
```

## Connect to red-monkey from a client:

```bash
$ redis-cli -p 6350

127.0.0.1:6350> set foo bar
OK
127.0.0.1:6350> get foo
"bar" 
```