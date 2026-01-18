# Actix-web Rust Server

A rewrite of the Node.js/Express server in Rust using Actix-web.

## Features

- Static file serving for Angular SPA
- CORS support
- Email sending via Resend API
- Environment-based configuration

## Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- The Angular client built at `../client/dist/ng-site/browser`

## Configuration

Copy `.env.example` to `.env` and configure:

```bash
cp .env.example .env
```

Required environment variables:
- `MAIL_API_KEY` - Your Resend API key
- `NODE_ENV` - Environment (development/production)

Optional:
- `PORT` - Server port (default: 8080)
- `RUST_LOG` - Log level (default: info)

## Running

### Development

```bash
cargo run
```

### Production

```bash
cargo build --release
./target/release/server
```

### Docker

```bash
docker build -t server .
docker run -p 8080:8080 --env-file .env server
```

## API Endpoints

### POST /email

Send a contact form email.

**Request Body:**
```json
{
  "sender": "user@example.com",
  "firstName": "John",
  "lastName": "Doe",
  "message": "Hello!"
}
```

**Response:**
```json
{
  "data": true
}
```

### GET /*

Serves the Angular SPA. All unmatched routes return `index.html`.
