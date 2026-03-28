FROM node:22-slim AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf.template /etc/nginx/templates/default.conf.template
ENV API_UPSTREAM=api-ownpulse-api:80
# Only substitute API_UPSTREAM — leave nginx variables ($host, $uri, etc.) intact
ENV NGINX_ENVSUBST_FILTER=API_UPSTREAM
EXPOSE 80
