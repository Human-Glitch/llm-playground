from typing import Any
import httpx

DEFAULT_USER_AGENT = "llm-mcp-server-template/1.0"

class HttpClient:
    def __init__(self, user_agent: str = DEFAULT_USER_AGENT):
        self.user_agent = user_agent

    async def get(self, url: str, accept: str) -> dict[str, Any] | None:
        headers = {
            "User-Agent": self.user_agent,
            "Accept": accept
        }
        async with httpx.AsyncClient() as client:
            try:
                response = await client.get(url, headers=headers, timeout=30.0)
                response.raise_for_status()
                return response.json()
            except Exception as e:
                print(f"Error: {e}")
                return None
