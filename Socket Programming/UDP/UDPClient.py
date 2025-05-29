from socket import *
servername= 'localhost'
serverPort= 12000
clientSocket = socket(AF_INET, SOCK_DGRAM)
message = input('Input lowercase sentence :')
# AF_INET is just indicationg that the underlying network is IPV4 and 
# SOCK_DGRM mentions that it is an UDP socket 
# we do not define the port number ourselves as we let it be decided by the OS
clientSocket.sendto(message.encode(),(servername,serverPort))
modifiedMessage,serverAddress= clientSocket.recvfrom(2048) # 2048 is the buffer size here 
print(modifiedMessage.decode())
clientSocket.close()