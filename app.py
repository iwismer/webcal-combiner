"""
Serve new combined calendar
"""

import os
import json
from flask import Flask, make_response
from combine_calendars import ExistingCalendar, generate_combined_calendar

app = Flask(__name__)

cwd = os.path.abspath(os.path.dirname(__file__))
with open(os.path.join(cwd, 'config.json')) as f:
    config = json.load(f)

served_calendars = {}
for calendar in config['calendars']:
    existing_calendars = []
    for sub_cal in calendar['calendars']:
        existing_calendar = ExistingCalendar(sub_cal['name'],
                                            sub_cal['description'],
                                            sub_cal['url'])
        existing_calendars.append(existing_calendar)
    served_calendars[calendar['name']] = existing_calendars

server_key = config["key"]

@app.route('/')
def empty_response():
    response = make_response('', 200)
    return response

@app.route("/calendar/<key>/<cal_name>")
def combine_calendar(key, cal_name):
    """
    Return combined calendar in ics format
    """
    if key != server_key:
        return make_response("Not Authorized", 401)
    if cal_name not in served_calendars:
        return make_response("Not Found", 404)
    response =  make_response(generate_combined_calendar(cal_name, served_calendars[cal_name]).serialize())
    response.headers["Content-Disposition"] = "attachment; filename=calendar.ics"
    response.headers['Content-Type'] = 'text/calendar'
    return response
