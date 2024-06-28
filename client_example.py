import json
url = "http://localhost:3030/connect"
headers = {"Content-Type": "application/json"}
#data = {"password": "", "user": "", "token": "58aca8a6-f33f-4e65-a7ce-00d6a3ed03bb"}
data = {"password": "", "user": "", "token": "c903b152-bda4-49f2-913e-6ab3f7bbc225"}

import asyncio
from time import process_time
from websockets.sync.client import connect
import requests
import threading
import sys
import json


text_recived_raw = {
        "payload":None,
        "destination":"1"
        }

class CanelaError(Exception):
    pass


def receiving_messages(websocket):
    while True:
        try:
            msg =websocket.recv()
            if msg:
                print_there("AAAAA"+msg, 10,10)
                #print("AAAAA"+msg)
            else:
                break
        except:
            break

def connect_to_websocket(jwt):
    uri = "ws://127.0.0.1:3030/ws"
    headers = {"jwt": jwt,  "user_token": data["token"]}

    #with websockets.connect(uri, extra_headers=headers) as websocket: #Why The difference in parameter names?
    with connect(uri,additional_headers=headers) as websocket:
        threading.Thread(target=receiving_messages, args=[websocket]).start()
        while True:
            message = input(">")
            if message=="break":break
            try:
              text_recived_raw["payload"]=message
              full = json.dumps(text_recived_raw)
              print(full)
              websocket.send(full)
              print("Mensaje Enviado")
            except:
                raise CanelaError("Error Enviando")
        websocket.close()


if __name__ == "__main__":
    response = requests.get(url, headers=headers, json=data)
    print(response)
    print(response.text)

    jwt = response.json().get("session_token")
    print(jwt)
    print("Intentando Conectarse al Servidor")

    connect_to_websocket(jwt)
