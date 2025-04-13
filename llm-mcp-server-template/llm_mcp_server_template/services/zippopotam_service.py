from typing import Any
from .http_client import HttpClient

API_BASE = "http://api.zippopotam.us"

class ZippopotamService:
    def __init__(self, http_client: HttpClient):
        self.http_client = http_client

    async def get_location(self, zip_code: int) -> dict[str, Any] | None:
        url = f"{API_BASE}/us/{zip_code}"
        return await self.http_client.get(url, "application/json")
