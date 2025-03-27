# HTTP Outcall Project
=========================

## Overview

The HTTP Outcall project provides a set of Rust crates for making HTTP requests from canisters on the Internet Computer. The project includes three main crates: 
- `ic-http-outcall-api` - common types.
- `ic-http-outcall-proxy-canister` - canister to process pending HTTP outcalls.
- `ic-http-outcall-proxy-client` - service to fetch pending HTTP requests from `ic-http-outcall-proxy-canister`, execute them, and send results back to the `ic-http-outcall-proxy-canister`.

## Usage
To use non-replicated HTTP outcalls there should be:
- `ic-http-outcall-proxy-canister` deployed.
- `ic-http-outcall-proxy-client` service running and configured to work with the proxy canister.
- The client agent should be set as allowed proxy on the proxy canister initialization.


If some client canister want to perform non-replicated request, it should:
1. Call the `http_outcall` update endpoint of the `ic-http-outcall-proxy-canister`, providing request data and a callback endpoint name. The call will return `RequestId` of the HTTP request.
2. Wait until the `ic-http-outcall-proxy-canister` call the callback endpoint with the given `RequestId`. The response will contain result of the HTTP request.