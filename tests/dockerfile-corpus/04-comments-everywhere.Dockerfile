# Build image for the API
# Maintained by nobody

FROM node:20-alpine   # pinned deliberately

# install deps first for layer caching
COPY package*.json ./
RUN npm ci

CMD ["node", "server.js"]
