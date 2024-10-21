FROM python:3-slim
RUN pip install -r requirements.txt
ADD app.py /app/app.py
ADD combine_calendars.py /app/combine_calendars.py
CMD gunicorn -w 4 -b 0.0.0.0:5000 app.app:app
