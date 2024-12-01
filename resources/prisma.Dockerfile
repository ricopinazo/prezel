FROM alpine:3.20.3

ENV DATABASE_URL=""

RUN apk add nodejs npm
RUN npm install -g prisma@5.22.0

ENTRYPOINT [ ]
CMD echo -e "datasource db {\n    provider = \"sqlite\"\n    url = \"file:$DATABASE_URL\"\n}" > schema.prisma && \
    prisma db pull --schema=schema.prisma && prisma studio --hostname=0.0.0.0 --port=80 --schema=schema.prisma --browser none
