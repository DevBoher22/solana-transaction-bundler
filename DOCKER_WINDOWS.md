# Docker Setup für Windows PowerShell

## Schritt-für-Schritt Anleitung für Windows

### 1. Repository klonen
```powershell
git clone https://github.com/DevBoher22/solana-transaction-bundler.git
cd solana-transaction-bundler
```

### 2. Docker Image bauen
```powershell
docker build -t solana-bundler:latest .
```

### 3. Container starten (PowerShell-Syntax)
```powershell
docker run -d `
  --name solana-bundler `
  -p 8080:8080 `
  -p 9090:9090 `
  -e RUST_LOG=info `
  solana-bundler:latest
```

**Oder als eine Zeile:**
```powershell
docker run -d --name solana-bundler -p 8080:8080 -p 9090:9090 -e RUST_LOG=info solana-bundler:latest
```

### 4. Service testen

**Health Check:**
```powershell
curl http://localhost:8080/v1/health
```

**Oder mit Invoke-WebRequest (falls curl nicht verfügbar):**
```powershell
Invoke-WebRequest -Uri http://localhost:8080/v1/health
```

**Service Info:**
```powershell
curl http://localhost:8080/v1/info
```

**Metriken:**
```powershell
curl http://localhost:9090/metrics
```

### 5. Container-Status prüfen

**Laufende Container anzeigen:**
```powershell
docker ps
```

**Container-Logs anzeigen:**
```powershell
docker logs solana-bundler
```

**Live-Logs verfolgen:**
```powershell
docker logs -f solana-bundler
```

### 6. In Container einsteigen (für Debugging)
```powershell
docker exec -it solana-bundler /bin/bash
```

**CLI im Container testen:**
```powershell
docker exec -it solana-bundler bundler-cli health
```

### 7. Container stoppen und aufräumen
```powershell
# Container stoppen
docker stop solana-bundler

# Container entfernen
docker rm solana-bundler

# Image entfernen (optional)
docker rmi solana-bundler:latest
```

## Alternative: Docker Compose

**Starten:**
```powershell
docker-compose up -d
```

**Logs anzeigen:**
```powershell
docker-compose logs -f bundler
```

**Stoppen:**
```powershell
docker-compose down
```

## Troubleshooting

### Problem: "Cargo.lock not found"
Das ist normal - Cargo.lock wird automatisch während des Builds generiert.

### Problem: PowerShell-Syntax-Fehler
Verwende Backticks (`) statt Backslashes (\) für Zeilenumbrüche in PowerShell.

### Problem: Port bereits belegt
```powershell
# Andere Ports verwenden
docker run -d --name solana-bundler -p 8081:8080 -p 9091:9090 -e RUST_LOG=info solana-bundler:latest
```

### Problem: curl nicht gefunden
```powershell
# Verwende Invoke-WebRequest stattdessen
Invoke-WebRequest -Uri http://localhost:8080/v1/health
```

## Erwartete Ausgabe

**Erfolgreicher Health Check:**
```json
{
  "status": "healthy",
  "timestamp": "2024-01-01T12:00:00Z",
  "components": {
    "rpc_client": "healthy",
    "fee_manager": "healthy",
    "signing_manager": "healthy"
  }
}
```

**Service Info:**
```json
{
  "name": "solana-bundler",
  "version": "0.1.0",
  "uptime_seconds": 123,
  "build_info": {
    "version": "0.1.0",
    "git_hash": "abc123",
    "build_date": "2024-01-01"
  }
}
```

## Nächste Schritte

1. **Konfiguration anpassen**: Bearbeite die Konfigurationsdatei für deine Bedürfnisse
2. **Echte Tests**: Verwende echte Solana RPC-Endpunkte und Keypairs
3. **Monitoring**: Öffne http://localhost:3000 für Grafana (bei Docker Compose)
4. **Integration**: Teste die API mit deinen eigenen Anwendungen
