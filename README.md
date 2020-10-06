# mitm-cache

Minimal proxy/cache implementation.

Example httpie request for [http://httpbin.org/uuid](http://httpbin.org/uuid):

```
http :8000/request/10/aHR0cDovL2h0dHBiaW4ub3JnL3V1aWQ= x-mitm:abc -v   
```

Notice that the url is base64-urlencoded. The *10* is the acceptable cache age and the x-mitm auth header can be set by the `MITM_KEY` environment variable, if no such variable is set every request will be accepted and handled (this is very bad unless the server is unreachable by attackers).