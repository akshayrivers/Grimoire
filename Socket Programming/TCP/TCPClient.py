from socket import *
servername= 'localhost'
serverPort= 12000
clientSocket = socket(AF_INET, SOCK_STREAM)
clientSocket.connect((servername,serverPort))

message = input('Input lowercase sentence :')
clientSocket.send(message.encode())
modifiedMessage= clientSocket.recv(1024)
print(modifiedMessage.decode())
clientSocket.close()