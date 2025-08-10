# Roy - a token well spent

Roy is a HTTP server compatible with the OpenAI platform format that simulates errors and rate limit data so you can
test your clients behaviour under weird circumstances. Once started, Roy will run the server on port 8000 and will
return responses using [Lorem Ipsum](https://www.lipsum.com/) dummy text.

Roy comes with a tokenizer, so that it can compute the number of tokens contained both in the request and in the
response with a decent approximation. The number of tokens will be used to set the proper headers in the response and
simulate real-world situation to test your clients. The number of requests are also tracked, so that Roy can set the
appropriate limits in the response headers.

## Basic usage

To run the server, just invoke `roy` from the command line. In this case, there will be no errors and Roy will
respond according to its default configuration values.
```sh
roy
```

## Control returned responses

Roy will return responses containing fragments of "Lorem Ipsum". The length of the responses will determined the
number of tokens consumed and can be controlled. The length of the response is measured in number of characters, not
tokens. If not specified, Roy returns a response of 250 characters.

To return a fixed-length response of 100 characters, invoke Roy like this:

```sh
roy --response-length 100
```

To have Roy return a response of random length N at each request, pass a range in the format `a:b` where a <= N <= b.
For example:

```sh
roy --response-length 10:100
```

## Returning errors

To simulate an error, you can pass the HTTP error code and the desired frequency in percent for that error to happen.
For example, to return a 429 error half of the times, you can invoke Roy like this:

```sh
roy --error-code 429 --error-rate 50
```

## Introducing rate limits

Roy can simulate actual rate limits by providing random responses while keeping track of the number of requests,
number of tokens and reset timing.

### Requests rate limits

Roy can simulate requests limits by setting the following headers in the response:

| Header | Description |
| ------ | ----------- |
| x-ratelimit-limit-requests | The maximum number of requests that are permitted before exhausting the rate limit. |
| x-ratelimit-remaining-requests The remaining number of requests that are permitted before exhausting the rate limit. |
| x-ratelimit-reset-requests | The time until the rate limit (based on requests) resets to its initial state. |

To control how Roy populates those headers, start the server passing the value for the desired limits:

```sh
roy --rpm 100
```

## Tokens rate limits

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
