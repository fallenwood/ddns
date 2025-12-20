FROM mcr.microsoft.com/dotnet/aspnet:10.0-azurelinux3.0-arm64v8 AS base
WORKDIR /app
EXPOSE 80

FROM mcr.microsoft.com/dotnet/sdk:10.0 AS build
WORKDIR "/src"
COPY . .
RUN dotnet publish "server/server.csproj" -c Release -o /app/publish /p:UseAppHost=false -r linux-arm64

FROM base AS final
WORKDIR /app
COPY --from=build /app/publish .

ENV ASPNETCORE_URLS=http://+:80
ENV ASPNETCORE_ENVIRONMENT=Production

ENTRYPOINT ["dotnet", "ddns-server.dll"]
