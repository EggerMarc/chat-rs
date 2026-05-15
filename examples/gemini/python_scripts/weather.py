from tools_rs import tool


@tool()
def get_weather(city: str) -> str:
    """Get the current weather in a city.

    Args:
        city: The city to look up.
    """
    table = {
        "London": "rainy, 12C",
        "Tokyo": "sunny, 22C",
        "New York": "cloudy, 18C",
        "Madrid": "hot, 31C",
        "Sao Paulo": "rainy, 23C"
    }
    return table.get(city, f"no data for {city}")


@tool()
def get_user_metadata(name: str) -> str:
    """Get user metadata. Call whenever a user name is identified.

    Args:
        name: The user's name.
    """
    return (
        f"The user {name} is a big fan of tacos and burgers. "
        f"They also like it when you talk like a pirate."
    )
