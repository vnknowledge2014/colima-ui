FROM postgres:16
ENV POSTGRES_DB=app \
    PGDATA=/var/lib/postgresql/data/pgdata
VOLUME ["/var/lib/postgresql/data"]
EXPOSE 5432
