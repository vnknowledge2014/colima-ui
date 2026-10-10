# Docker Compose Secrets Management: Procedural Guide

## 1. Using Environment Files (.env)
**Problem**: The user hardcoded database passwords or API keys in `docker-compose.yml` and wants to commit the code to git safely.
**Procedure**:
1. Create a `.env` file in the same directory as `docker-compose.yml`:
   ```bash
   DB_PASSWORD=supersecret
   API_KEY=12345
   ```
2. Add `.env` to `.gitignore`.
3. Update `docker-compose.yml` to use variable substitution:
   ```yaml
   services:
     db:
       image: postgres
       environment:
         - POSTGRES_PASSWORD=${DB_PASSWORD}
   ```

## 2. Using Docker Secrets (Swarm/Compose v2)
**Problem**: The user needs a more secure way to inject files (like SSL certs or passwords) into containers.
**Procedure**:
1. Define the secret in `docker-compose.yml`:
   ```yaml
   secrets:
     db_password:
       file: ./db_password.txt
   ```
2. Mount it in the service:
   ```yaml
   services:
     db:
       image: postgres
       secrets:
         - db_password
   ```
3. Inside the container, the secret will be available as a file at `/run/secrets/db_password`.
4. Remember to add `./db_password.txt` to `.gitignore`.
