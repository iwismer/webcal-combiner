"""
Class and function for combining existing ics calendars
"""
import logging
from dataclasses import dataclass
from icalendar import Calendar, Event, Timezone
from icalendar.cal import Component
import requests


@dataclass
class ExistingCalendar:
    """
    Dataclass to represent an existing calendar
    """
    name: str
    description: str
    url: str


def get_calendar(url) -> str:
    resp = requests.get(url, timeout=30)
    resp.raise_for_status()
    return resp.text

def generate_combined_calendar(name: str, calendars: list):
    """
    Generate a new calendar with events from the exsting calendars

    :param name: Name of new calendar
    :param calendars: List of `ExistingCalendar()` objects
    :return: New `ics.Calendar()`
    """
    new_cal = Calendar()
    new_cal.add('prodid', name)
    new_cal.add('version', '2.0')
    new_cal.add('NAME', name)
    new_cal.add('X-WR-CALNAME', name)

    for calendar in calendars:
        raw_calendar = get_calendar(calendar.url)
        calendar_content = Calendar().from_ical(raw_calendar)

        for component in calendar_content.subcomponents:
            if isinstance(component, Timezone):
                new_cal.add_component(component)
            elif isinstance(component, Event):
                component['summary'] = component['summary'] + f" [{calendar.name}]"
                new_cal.add_component(component)

    return new_cal.to_ical().decode()
