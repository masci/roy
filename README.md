# Roy - a token well spent

<p align="center">
<img width="700" height="140" alt="roy videogame" src="https://github.com/user-attachments/assets/81801b89-ba5e-4122-82b3-29743aa11147" />
</p>

Roy is a HTTP server compatible with the OpenAI platform format that simulates errors and rate limit data so you can
test your clients behaviour under weird circumstances. Once started, Roy will run the server on port 8000 and will
return responses using [Lorem Ipsum](https://www.lipsum.com/) dummy text.

## :floppy_disk: Installation

If you have Rust available, you can install Roy from [crates.io](https://crates.io/) with:
```
cargo install roy-cli
```

Alternatively, you can download one of the pre-compiled binaries from the
[latest release](https://github.com/masci/roy/releases) on GitHub, choosing the right archive for your platform.

To run the server, just invoke `roy` from the command line. In this case, there will be no errors and Roy will
respond according to its default configuration values.
```sh
roy
# Roy server running on http://127.0.0.1:8000
```

## :memo: Control text responses

Roy will return responses containing fragments of "Lorem Ipsum". The length of the responses will determined the
number of tokens consumed and can be controlled. The length of the response is measured in number of characters, not
tokens. If not specified, Roy returns a response of 250 characters.

### Always return a certain amount of text

To return a fixed-length response of 100 characters, invoke Roy like this:

```sh
roy --response-length 100
```

### Return a random amount of text within a certain size interval

To have Roy return a response of random length N at each request, pass a range in the format `a:b` where a <= N <= b.
For example:

```sh
roy --response-length 10:100
```

## :boom: Simulate errors

### HTTP Errors

To simulate an error, you can pass the HTTP error code and the desired frequency in percent for that error to happen.
For example, to return a 429 error half of the times, you can invoke Roy like this:

```sh
roy --error-code 429 --error-rate 50
```

### Timeout errors

OpenAI has a default timeout for requests of 10 minutes. To easily simulate a timeout scenario without changing the
client code, you can tell Roy to time out requests after a certain amount of time expressed in milliseconds:

```sh
roy --timeout 500
```

### Slow responses

You can simulate slow responses by having Roy introduce a sleep before responding to the HTTP request. You can either
pass a fixed amount of milliseconds that will be wasted for each and every request:

```sh
roy --slowdown 100
```

Or you can introduce random slowness between a range of milliseconds:

```sh
roy --slowdown 0:1000
```

## :control_knobs: Control rate limits

Roy comes with a tokenizer, so that it can compute the number of tokens contained both in the request and in the
response with a decent approximation. The number of tokens will be used to set the proper headers in the response and
simulate real-world situation to test your clients. The number of requests are also tracked, so that Roy can set the
appropriate limits in the response headers.

### Requests rate limits

Roy can simulate requests limits by setting the following headers in the response:

| Header | Description |
| ------ | ----------- |
| x-ratelimit-limit-requests | The maximum number of requests that are permitted before exhausting the rate limit. |
| x-ratelimit-remaining-requests | The remaining number of requests that are permitted before exhausting the rate limit. |
| x-ratelimit-reset-requests | The time until the rate limit (based on requests) resets to its initial state. |

To control how Roy populates those headers, start the server passing the value for the desired limits:

```sh
roy --rpm 100
```

### Tokens rate limits

Roy can simulate token limits by setting the following headers in the response:

| Header | Description |
| ------ | ----------- |
| x-ratelimit-limit-tokens | The maximum number of tokens that are permitted before exhausting the rate limit. |
| x-ratelimit-remaining-tokens | The remaining number of tokens that are permitted before exhausting the rate limit. |
| x-ratelimit-reset-tokens | The time until the rate limit (based on tokens) resets to its initial state. |

To set the tokens per minute limit:

```sh
roy --tpm 45000
```

## :card_index_dividers: Supported APIs

- https://platform.openai.com/docs/api-reference/responses/create
- https://platform.openai.com/docs/api-reference/chat/create
