from typing import Any
from .http_client import HttpClient

API_BASE = "https://api.weather.gov"

class NWSService:
    def __init__(self, http_client: HttpClient):
        self.http_client = http_client

    async def get_alerts(self, state: str) -> dict[str, Any] | None:
        url = f"{API_BASE}/alerts/active/area/{state}"
        return await self.http_client.get(url, "application/geo+json")

    async def get_points(self, lat: str, lon: str) -> dict[str, Any] | None:
        url = f"{API_BASE}/points/{lat},{lon}"
        return await self.http_client.get(url, "application/geo+json")

    async def get_forecast(self, forecast_url: str) -> dict[str, Any] | None:
        return await self.http_client.get(forecast_url, "application/geo+json")
