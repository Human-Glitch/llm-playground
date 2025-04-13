from typing import Any
from mcp.server.fastmcp import FastMCP
from services.http_client import HttpClient
from services.nws_service import NWSService
from services.zippopotam_service import ZippopotamService

# Initialize FastMCP server
mcp = FastMCP("llm-mcp-server-template")

# Initialize services
http_client = HttpClient()
nws_service = NWSService(http_client)
zip_service = ZippopotamService(http_client)

def format_alert(feature: dict) -> str:
    """Format an alert feature into a readable string."""
    props = feature["properties"]
    return f"""
    Event: {props.get('event', 'Unknown')}
    Area: {props.get('areaDesc', 'Unknown')}
    Severity: {props.get('severity', 'Unknown')}
    Description: {props.get('description', 'No description available')}
    Instructions: {props.get('instruction', 'No specific instructions provided')}
    """

@mcp.tool()
async def get_weather_alerts(state: str) -> str:
    """Get weather alerts for a US state.

    Args:
        state: Two-letter US state code (e.g. CA, NY)
    """
    data = await nws_service.get_alerts(state)
    
    if not data or "features" not in data:
        return "Unable to fetch alerts or no alerts found."

    if not data["features"]:
        return "No active alerts for this state."

    alerts = [format_alert(feature) for feature in data["features"]]
    return "\n---\n".join(alerts)

@mcp.tool()
async def get_weather_forecast(zipCode: int) -> str:
    """Get weather forecast for a location.

    Args:
        zipCode: The zip code of the location
    """
    zip_data = await zip_service.get_location(zipCode)
    if not zip_data:
        return "Unable to fetch zip data for this location."
    
    longitude = zip_data["places"][0]["longitude"]
    latitude = zip_data["places"][0]["latitude"]

    points_data = await nws_service.get_points(latitude, longitude)
    if not points_data:
        return "Unable to fetch forecast data for this location."

    forecast_url = points_data["properties"]["forecast"]
    forecast_data = await nws_service.get_forecast(forecast_url)
    if not forecast_data:
        return "Unable to fetch detailed forecast."

    # Format the periods into a readable forecast
    periods = forecast_data["properties"]["periods"]
    forecasts = []
    for period in periods[:5]:
        forecast = f"""{period['name']}:
        Temperature: {period['temperature']}°{period['temperatureUnit']}
        Wind: {period['windSpeed']} {period['windDirection']}
        Forecast: {period['detailedForecast']}
        """
        forecasts.append(forecast)

    return "\n---\n".join(forecasts)

if __name__ == "__main__":
    mcp.run(transport='stdio')