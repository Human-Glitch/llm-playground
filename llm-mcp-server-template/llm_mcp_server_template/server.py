from typing import Dict, Any, Optional
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
async def get_weather_forecast(zip_code: int) -> str:
    """Get weather forecast for a location.

    Args:
        zip_code: The zip code of the location
    """
    zip_data = await zip_service.get_location(zip_code)
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

@mcp.resource("prompts://format_location_data")
def get_format_location_data() -> str:
    with open("prompts/format_location_data.txt", "r", encoding="utf-8") as file:
        file_prompt = file.read()

    return file_prompt

@mcp.prompt()
async def format_location_data(context=None, zip_code: int = None) -> str:
    """Format location data into a specific structure using model context.
    This demonstrates how to guide an LLM to format data consistently.

    Args:
        zip_code: The zip code to look up
    """
    if zip_code is None:
        return "zip_code is required."

    zip_data = await zip_service.get_location(zip_code)
    if not zip_data:
        return "Unable to fetch zip data for this location."

    prompt = f"{get_format_location_data()}\n\n{zip_data}"

    if not context:
        return f"No model context available to process this prompt: {prompt}."

    try:
        # This sends a prompt back to the LLM and gets its response
        response = await context.prompt(
            prompt=prompt,
            temperature=0.7,
            max_tokens=500
        )
        return response
    except Exception as e:
        return f"Error when prompting LLM: {str(e)}"

if __name__ == "__main__":
    mcp.run(transport='stdio')