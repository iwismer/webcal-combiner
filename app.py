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
url = config['url']

@app.route('/')
def empty_response():
    response = make_response('', 200)
    return response


@app.route('/listing')
def listing():
    res_str = ""
    for cal, sub_cals in served_calendars.items():
        res_str = res_str + f"{cal}\n{url}/calendar/{server_key}/{cal}\n\n"
        for sub_cal in sub_cals:
            res_str = res_str + f"{sub_cal.name}: {sub_cal.description}\n{sub_cal.url}\n"
        res_str = res_str + "\n------\n\n"
    response = make_response(res_str, 200)
    response.headers['Content-Type'] = 'text/plain'
    return response

def combine_all_calendars():
    """
    Return all the calendars in a single ics file
    """
    
    combined = []
    for cal in served_calendars.keys():
        generated_cal = generate_combined_calendar(cal, served_calendars[cal])
        combined.append(generated_cal)
    response =  make_response('\n'.join(combined))
    response.headers["Content-Disposition"] = "attachment; filename=all-calendars.ics"
    response.headers['Content-Type'] = 'text/calendar'
    return response

@app.route("/calendar/<key>/<cal_name>")
def combine_calendar(key, cal_name):
    """
    Return combined calendar in ics format
    """
    if key != server_key:
        return make_response("Not Authorized", 401)
    if cal_name == 'all-calendars':
        return combine_all_calendars()
    elif cal_name not in served_calendars:
        return make_response("Not Found", 404)
    response =  make_response(generate_combined_calendar(cal_name, served_calendars[cal_name]))
    response.headers["Content-Disposition"] = "attachment; filename=calendar.ics"
    response.headers['Content-Type'] = 'text/calendar'
    return response