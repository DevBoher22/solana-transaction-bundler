# Docker Quick Start Guide

## Voraussetzungen

- Docker Desktop installiert und gestartet
- Git (um das Repository zu klonen)

## Schritt 1: Repository klonen

```bash
git clone https://github.com/DevBoher22/solana-transaction-bundler.git
cd solana-transaction-bundler
```

## Schritt 2: Docker Image bauen

```bash
# Baue das Docker Image
docker build -t solana-bundler:latest .

# Oder verwende den Make-Befehl
make docker-build
```

## Schritt 3: Konfiguration vorbereiten

```bash
# Kopiere die Beispielkonfiguration
cp examples/bundler.config.toml local.config.toml

# Bearbeite die Konfiguration für lokale Tests (optional)
# Die Standardkonfiguration sollte für erste Tests funktionieren
```

## Schritt 4: Container starten

### Option A: Einfacher Start (nur HTTP Service)

```bash
docker run -d \
  --name solana-bundler \
  -p 8080:8080 \
  -p 9090:9090 \
  -e RUST_LOG=info \
  -v $(pwd)/local.config.toml:/app/bundler.config.toml:ro \
  solana-bundler:latest
```

### Option B: Mit Docker Compose (empfohlen)

```bash
# Starte alle Services (Bundler + Monitoring)
docker-compose up -d

# Logs anzeigen
docker-compose logs -f bundler
```

## Schritt 5: Service testen

### Health Check

```bash
# Grundlegende Gesundheitsprüfung
curl http://localhost:8080/v1/health

# Detaillierte Gesundheitsprüfung
curl http://localhost:8080/v1/health?verbose=true | jq .
```

### Service Info

```bash
# Service-Informationen abrufen
curl http://localhost:8080/v1/info | jq .
```

### Metriken anzeigen

```bash
# Prometheus-Metriken
curl http://localhost:9090/metrics
```

## Schritt 6: CLI testen (optional)

```bash
# CLI im Container ausführen
docker exec -it solana-bundler bundler-cli health

# Konfiguration validieren
docker exec -it solana-bundler bundler-cli config --validate
```

## Monitoring (mit Docker Compose)

Wenn du Docker Compose verwendest, sind zusätzliche Services verfügbar:

- **Bundler Service**: http://localhost:8080
- **Prometheus**: http://localhost:9091
- **Grafana**: http://localhost:3000 (admin/admin)

## Troubleshooting

### Container-Logs anzeigen

```bash
# Docker run
docker logs solana-bundler

# Docker Compose
docker-compose logs bundler
```

### Container-Status prüfen

```bash
# Laufende Container
docker ps

# Container-Details
docker inspect solana-bundler
```

### In Container einsteigen

```bash
# Shell im Container
docker exec -it solana-bundler /bin/sh

# Dateien im Container prüfen
docker exec -it solana-bundler ls -la /app/
```

### Häufige Probleme

**Port bereits belegt:**
```bash
# Andere Ports verwenden
docker run -p 8081:8080 -p 9091:9090 ...
```

**Konfigurationsfehler:**
```bash
# Konfiguration validieren
docker exec -it solana-bundler bundler-cli config --validate
```

**Speicher-/CPU-Probleme:**
```bash
# Ressourcen-Limits setzen
docker run --memory=1g --cpus=0.5 ...
```

## Erweiterte Tests

### Bundle-Simulation testen

```bash
# Beispiel-Bundle simulieren (benötigt gültige Solana-Daten)
curl -X POST http://localhost:8080/v1/bundle/simulate \
  -H "Content-Type: application/json" \
  -d @examples/bundle_request.json
```

### Performance-Test

```bash
# Mehrere parallele Requests
for i in {1..10}; do
  curl -s http://localhost:8080/v1/health &
done
wait
```

## Aufräumen

```bash
# Container stoppen und entfernen
docker stop solana-bundler
docker rm solana-bundler

# Oder mit Docker Compose
docker-compose down

# Image entfernen (optional)
docker rmi solana-bundler:latest
```

## Nächste Schritte

1. **Konfiguration anpassen**: Bearbeite `local.config.toml` für deine Bedürfnisse
2. **Echte Solana-Daten**: Konfiguriere RPC-Endpunkte und Keypairs
3. **Monitoring**: Nutze Grafana-Dashboards für detaillierte Metriken
4. **Integration**: Teste die API mit deinen eigenen Anwendungen

## Hilfe

Bei Problemen:
1. Prüfe die Container-Logs
2. Validiere die Konfiguration
3. Teste die Health-Endpunkte
4. Erstelle ein Issue auf GitHub: https://github.com/DevBoher22/solana-transaction-bundler/issues
